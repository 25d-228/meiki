INSERT OR IGNORE INTO scheduler_parameter_sets(
    id,
    engine_version,
    parameters_json,
    created_at_ms
) VALUES (
    'fsrs7-default-v1',
    'fsrs-7',
    '[0.041,2.4175,4.1283,11.9709,5.6385,0.4468,3.262,2.3054,0.1688,1.3325,0.3524,0.0049,0.7503,0.0896,0.6625,1.3,0.882,0.3072,3.5875,0.303,0.0107,0.2279,2.6413,0.5594,1.3,2.5,1.0,0.0723,0.1634,0.5,0.9555,0.2245,0.6232,0.1362,0.3862]',
    unixepoch('subsec') * 1000
);

CREATE TABLE scheduler_profiles (
    deck_id TEXT PRIMARY KEY REFERENCES decks(id) ON DELETE CASCADE,
    engine_version TEXT NOT NULL,
    active_parameter_set_id TEXT NOT NULL
        REFERENCES scheduler_parameter_sets(id) ON DELETE RESTRICT,
    previous_parameter_set_id TEXT
        REFERENCES scheduler_parameter_sets(id) ON DELETE RESTRICT,
    intensity TEXT NOT NULL DEFAULT 'balanced'
        CHECK (intensity IN ('light', 'balanced', 'intensive')),
    daily_time_budget_minutes INTEGER
        CHECK (daily_time_budget_minutes IS NULL OR daily_time_budget_minutes > 0),
    day_boundary_minutes INTEGER NOT NULL DEFAULT 240
        CHECK (day_boundary_minutes BETWEEN 0 AND 1439),
    optimizer_status TEXT NOT NULL DEFAULT 'never_run'
        CHECK (
            optimizer_status IN (
                'never_run',
                'insufficient_data',
                'adopted',
                'rejected',
                'failed',
                'rolled_back'
            )
        ),
    optimizer_diagnostics TEXT,
    updated_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO scheduler_profiles(
    deck_id,
    engine_version,
    active_parameter_set_id,
    intensity,
    day_boundary_minutes,
    optimizer_status,
    updated_at_ms
)
SELECT
    id,
    'fsrs-7',
    'fsrs7-default-v1',
    'balanced',
    240,
    'never_run',
    unixepoch('subsec') * 1000
FROM decks;

ALTER TABLE schedule_states
ADD COLUMN ideal_due_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE schedule_states
ADD COLUMN interval_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (interval_milliseconds >= 0);

ALTER TABLE schedule_states
ADD COLUMN stability_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (stability_milliseconds >= 0);

ALTER TABLE schedule_states
ADD COLUMN difficulty_millipoints INTEGER NOT NULL DEFAULT 0
    CHECK (
        difficulty_millipoints = 0
        OR difficulty_millipoints BETWEEN 1000 AND 10000
    );

ALTER TABLE schedule_states
ADD COLUMN last_reviewed_at_ms INTEGER;

UPDATE schedule_states
SET
    ideal_due_at_ms = due_at_ms,
    interval_milliseconds = interval_seconds * 1000;

ALTER TABLE schedule_baselines
ADD COLUMN ideal_due_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE schedule_baselines
ADD COLUMN interval_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (interval_milliseconds >= 0);

ALTER TABLE schedule_baselines
ADD COLUMN stability_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (stability_milliseconds >= 0);

ALTER TABLE schedule_baselines
ADD COLUMN difficulty_millipoints INTEGER NOT NULL DEFAULT 0
    CHECK (
        difficulty_millipoints = 0
        OR difficulty_millipoints BETWEEN 1000 AND 10000
    );

ALTER TABLE schedule_baselines
ADD COLUMN last_reviewed_at_ms INTEGER;

UPDATE schedule_baselines
SET
    ideal_due_at_ms = due_at_ms,
    interval_milliseconds = interval_seconds * 1000;

ALTER TABLE review_events
ADD COLUMN target_retention_basis_points INTEGER NOT NULL DEFAULT 9000
    CHECK (target_retention_basis_points BETWEEN 7000 AND 9900);

ALTER TABLE review_events
ADD COLUMN previous_ideal_due_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE review_events
ADD COLUMN previous_interval_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (previous_interval_milliseconds >= 0);

ALTER TABLE review_events
ADD COLUMN previous_stability_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (previous_stability_milliseconds >= 0);

ALTER TABLE review_events
ADD COLUMN previous_difficulty_millipoints INTEGER NOT NULL DEFAULT 0
    CHECK (
        previous_difficulty_millipoints = 0
        OR previous_difficulty_millipoints BETWEEN 1000 AND 10000
    );

ALTER TABLE review_events
ADD COLUMN previous_last_reviewed_at_ms INTEGER;

ALTER TABLE review_events
ADD COLUMN next_ideal_due_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE review_events
ADD COLUMN next_interval_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (next_interval_milliseconds >= 0);

ALTER TABLE review_events
ADD COLUMN next_stability_milliseconds INTEGER NOT NULL DEFAULT 0
    CHECK (next_stability_milliseconds >= 0);

ALTER TABLE review_events
ADD COLUMN next_difficulty_millipoints INTEGER NOT NULL DEFAULT 0
    CHECK (
        next_difficulty_millipoints = 0
        OR next_difficulty_millipoints BETWEEN 1000 AND 10000
    );

ALTER TABLE review_events
ADD COLUMN next_last_reviewed_at_ms INTEGER;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (4, unixepoch('subsec') * 1000);
