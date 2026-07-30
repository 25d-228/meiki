#!/usr/bin/env python3
"""Generate committed FSRS-7 parity vectors from the pinned scalar equations."""

from __future__ import annotations

import json
import math
from pathlib import Path

REFERENCE_COMMIT = "70cc4387f573ff20b13ac9c106333a335c8a4cb8"
DAY_MS = 86_400_000
MIN_STABILITY_DAYS = 0.0001
MAX_STABILITY_DAYS = 36_500.0
DEFAULT_PARAMETERS = [
    0.041,
    2.4175,
    4.1283,
    11.9709,
    5.6385,
    0.4468,
    3.262,
    2.3054,
    0.1688,
    1.3325,
    0.3524,
    0.0049,
    0.7503,
    0.0896,
    0.6625,
    1.3,
    0.882,
    0.3072,
    3.5875,
    0.303,
    0.0107,
    0.2279,
    2.6413,
    0.5594,
    1.3,
    2.5,
    1.0,
    0.0723,
    0.1634,
    0.5,
    0.9555,
    0.2245,
    0.6232,
    0.1362,
    0.3862,
]
CUSTOM_PARAMETERS = [
    0.08,
    1.8,
    5.0,
    14.0,
    6.0,
    0.35,
    2.8,
    2.8,
    0.2,
    1.1,
    0.4,
    0.01,
    0.7,
    0.12,
    0.7,
    1.5,
    1.1,
    0.4,
    3.2,
    0.35,
    0.02,
    0.3,
    2.4,
    0.6,
    1.4,
    3.0,
    0.85,
    0.08,
    0.2,
    0.6,
    0.96,
    0.3,
    0.7,
    0.2,
    0.45,
]
GRADES = {"again": 1, "hard": 2, "good": 3, "easy": 4}


def round_positive(value: float) -> int:
    return math.floor(value + 0.5)


def forgetting_curve(parameters: list[float], elapsed_days: float, stability: float) -> float:
    stability = max(stability, MIN_STABILITY_DAYS)
    ratio = max(elapsed_days, 0.0) / stability

    def power_law(base: float, decay: float) -> float:
        factor = base ** (1.0 / decay) - 1.0
        return (1.0 + factor * ratio) ** decay

    first = power_law(parameters[29], -parameters[27])
    second = power_law(parameters[30], -parameters[28])
    first_weight = parameters[31] * stability ** (-parameters[33])
    second_weight = parameters[32] * stability ** parameters[34]
    return min(
        1.0,
        max(
            0.0,
            (first_weight * first + second_weight * second)
            / (first_weight + second_weight),
        ),
    )


def initial_difficulty(parameters: list[float], rating: int) -> float:
    return parameters[4] - math.exp(parameters[5] * (rating - 1.0)) + 1.0


def stability_after_review(
    parameters: list[float],
    stability: float,
    difficulty: float,
    retrievability: float,
    rating: int,
    base: int,
) -> float:
    failed = (
        parameters[base + 3]
        * difficulty ** (-parameters[base + 4])
        * ((stability + 1.0) ** parameters[base + 5] - 1.0)
        * math.exp((1.0 - retrievability) * parameters[base + 6])
    )
    post_lapse = min(stability, failed)
    if rating == 1:
        return post_lapse
    hard_penalty = parameters[base + 7] if rating == 2 else 1.0
    easy_bonus = parameters[base + 8] if rating == 4 else 1.0
    increase = (
        1.0
        + math.exp(parameters[base] - 1.5)
        * (11.0 - difficulty)
        * stability ** (-parameters[base + 1])
        * (math.exp((1.0 - retrievability) * parameters[base + 2]) - 1.0)
        * hard_penalty
        * easy_bonus
    )
    return max(post_lapse, stability * increase)


def interval_ms(parameters: list[float], stability: float, target: float) -> int:
    low = 0.0
    high = MAX_STABILITY_DAYS
    if forgetting_curve(parameters, high, stability) > target:
        return round_positive(high * DAY_MS)
    for _ in range(80):
        midpoint = (low + high) / 2.0
        if forgetting_curve(parameters, midpoint, stability) > target:
            low = midpoint
        else:
            high = midpoint
    return max(60_000, round_positive(((low + high) / 2.0) * DAY_MS))


def generate_case(
    name: str,
    parameters_name: str,
    target_basis_points: int,
    transitions: list[tuple[str, int]],
) -> dict[str, object]:
    parameters = (
        DEFAULT_PARAMETERS if parameters_name == "default" else CUSTOM_PARAMETERS
    )
    stability: float | None = None
    difficulty: float | None = None
    reviewed_at_ms = 0
    repetitions = 0
    steps: list[dict[str, object]] = []
    for grade_name, elapsed_ms in transitions:
        rating = GRADES[grade_name]
        reviewed_at_ms += elapsed_ms
        if stability is None or difficulty is None:
            stability = parameters[rating - 1]
            difficulty = min(10.0, max(1.0, initial_difficulty(parameters, rating)))
        else:
            elapsed_days = elapsed_ms / DAY_MS
            retrievability = forgetting_curve(parameters, elapsed_days, stability)
            long_term = stability_after_review(
                parameters, stability, difficulty, retrievability, rating, 7
            )
            short_term = stability_after_review(
                parameters, stability, difficulty, retrievability, rating, 16
            )
            transition = min(
                1.0,
                max(
                    0.0,
                    1.0
                    - parameters[26]
                    * math.exp(-parameters[25] * elapsed_days),
                ),
            )
            stability = min(
                MAX_STABILITY_DAYS,
                max(
                    MIN_STABILITY_DAYS,
                    transition * long_term + (1.0 - transition) * short_term,
                ),
            )
            delta = -parameters[6] * (rating - 3.0)
            damped = delta * (10.0 - difficulty) / 9.0
            difficulty = min(
                10.0,
                max(
                    1.0,
                    0.01 * initial_difficulty(parameters, 4)
                    + 0.99 * (difficulty + damped),
                ),
            )

        stored_stability_ms = round_positive(stability * DAY_MS)
        stored_difficulty = round_positive(difficulty * 1_000.0)
        stored_stability = stored_stability_ms / DAY_MS
        stored_difficulty_value = stored_difficulty / 1_000.0
        interval = interval_ms(
            parameters, stability, target_basis_points / 10_000.0
        )
        repetitions = 0 if rating == 1 else repetitions + 1
        recall_after_30_days = forgetting_curve(
            parameters, 30.0, stored_stability
        )
        steps.append(
            {
                "grade": grade_name,
                "elapsed_ms": elapsed_ms,
                "reviewed_at_ms": reviewed_at_ms,
                "stability_milliseconds": stored_stability_ms,
                "difficulty_millipoints": stored_difficulty,
                "interval_milliseconds": interval,
                "due_at_ms": reviewed_at_ms + interval,
                "repetitions": repetitions,
                "recall_after_30_days": recall_after_30_days,
            }
        )
        stability = stored_stability
        difficulty = stored_difficulty_value

    return {
        "name": name,
        "parameters": parameters_name,
        "target_retention_basis_points": target_basis_points,
        "steps": steps,
    }


def cases() -> list[dict[str, object]]:
    generated: list[dict[str, object]] = []
    elapsed = [
        ("same-minute", 60_000),
        ("same-hour", 60 * 60 * 1_000),
        ("same-day", 12 * 60 * 60 * 1_000),
        ("normal", 7 * DAY_MS),
        ("overdue", 365 * DAY_MS),
        ("very-long", 20_000 * DAY_MS),
    ]
    for first in GRADES:
        generated.append(
            generate_case(f"first-{first}", "default", 9_000, [(first, 0)])
        )
        for elapsed_name, elapsed_ms in elapsed:
            for subsequent in GRADES:
                generated.append(
                    generate_case(
                        f"{first}-{subsequent}-{elapsed_name}",
                        "default",
                        9_000,
                        [(first, 0), (subsequent, elapsed_ms)],
                    )
                )

    mixed = [
        ("good", 0),
        ("good", DAY_MS // 4),
        ("again", 7 * DAY_MS),
        ("hard", 0),
        ("easy", 120 * DAY_MS),
        ("again", 365 * DAY_MS),
        ("good", 60_000),
        ("easy", 20_000 * DAY_MS),
    ]
    for target in [7_000, 8_000, 8_500, 9_000, 9_500, 9_900]:
        generated.append(
            generate_case(f"mixed-target-{target}", "default", target, mixed)
        )
    for target in [7_000, 8_500, 9_000, 9_500, 9_900]:
        generated.append(
            generate_case(f"custom-target-{target}", "custom", target, mixed)
        )
    generated.append(
        generate_case(
            "repeated-lapses",
            "default",
            9_000,
            [
                ("again", 0),
                ("again", 60_000),
                ("again", 60 * 60 * 1_000),
                ("good", DAY_MS),
                ("again", 30 * DAY_MS),
                ("again", 365 * DAY_MS),
            ],
        )
    )
    return generated


def main() -> None:
    destination = Path(__file__).with_name("fsrs7-reference.json")
    payload = {
        "reference_commit": REFERENCE_COMMIT,
        "generator": "python crates/meiki-scheduler/fixtures/generate_fsrs7_reference.py",
        "precision": "IEEE-754 binary64 scalar equations; persisted integer fields use nearest rounding",
        "recall_tolerance": 1e-12,
        "parameter_sets": {
            "default": DEFAULT_PARAMETERS,
            "custom": CUSTOM_PARAMETERS,
        },
        "cases": cases(),
    }
    destination.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    print(f"wrote {len(payload['cases'])} cases to {destination}")


if __name__ == "__main__":
    main()
