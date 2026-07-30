CREATE TABLE collection_scheduler_settings (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    daily_time_budget_minutes INTEGER NOT NULL
        CHECK (daily_time_budget_minutes BETWEEN 1 AND 1440),
    updated_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO collection_scheduler_settings(
    singleton,
    daily_time_budget_minutes,
    updated_at_ms
)
SELECT
    1,
    MIN(
        1440,
        MAX(
            1,
            COALESCE(
                (
                    SELECT daily_time_budget_minutes
                    FROM scheduler_profiles
                    WHERE deck_id = 'default-deck'
                ),
                (
                    SELECT CASE intensity
                        WHEN 'light' THEN 15
                        WHEN 'intensive' THEN 60
                        ELSE 30
                    END
                    FROM scheduler_profiles
                    ORDER BY deck_id
                    LIMIT 1
                ),
                30
            )
        )
    ),
    unixepoch('subsec') * 1000;

ALTER TABLE scheduler_profiles
ADD COLUMN scheduling_mode TEXT NOT NULL DEFAULT 'automatic'
    CHECK (scheduling_mode IN ('automatic', 'expert'));

ALTER TABLE scheduler_profiles
ADD COLUMN controller_version TEXT NOT NULL DEFAULT 'time-budget-v1'
    CHECK (controller_version <> '');

ALTER TABLE scheduler_profiles
ADD COLUMN controller_target_retention_basis_points INTEGER NOT NULL DEFAULT 9000
    CHECK (controller_target_retention_basis_points BETWEEN 8000 AND 9500);

ALTER TABLE scheduler_profiles
ADD COLUMN controller_new_cards_per_day INTEGER NOT NULL DEFAULT 20
    CHECK (controller_new_cards_per_day BETWEEN 0 AND 10000);

ALTER TABLE scheduler_profiles
ADD COLUMN controller_last_evaluated_day_start_ms INTEGER;

ALTER TABLE scheduler_profiles
ADD COLUMN controller_review_count INTEGER NOT NULL DEFAULT 0
    CHECK (controller_review_count >= 0);

ALTER TABLE scheduler_profiles
ADD COLUMN controller_unseen_count INTEGER NOT NULL DEFAULT 0
    CHECK (controller_unseen_count >= 0);

ALTER TABLE scheduler_profiles
ADD COLUMN controller_forecast_review_seconds_per_day INTEGER NOT NULL DEFAULT 0
    CHECK (controller_forecast_review_seconds_per_day >= 0);

ALTER TABLE scheduler_profiles
ADD COLUMN controller_backlog_exceeds_budget INTEGER NOT NULL DEFAULT 0
    CHECK (controller_backlog_exceeds_budget IN (0, 1));

ALTER TABLE scheduler_profiles
ADD COLUMN controller_explanation TEXT NOT NULL DEFAULT '';

UPDATE scheduler_profiles
SET scheduling_mode = CASE
        WHEN EXISTS (
            SELECT 1
            FROM decks
            WHERE decks.id = scheduler_profiles.deck_id
              AND (
                  decks.target_retention_basis_points IS NOT NULL
                  OR decks.new_cards_per_day IS NOT NULL
                  OR decks.maximum_interval_days IS NOT NULL
              )
        )
        THEN 'expert'
        ELSE 'automatic'
    END,
    controller_target_retention_basis_points = MIN(
        9500,
        MAX(
            8000,
            COALESCE(
                (
                    SELECT decks.target_retention_basis_points
                    FROM decks
                    WHERE decks.id = scheduler_profiles.deck_id
                ),
                CASE intensity
                    WHEN 'light' THEN 8500
                    WHEN 'intensive' THEN 9300
                    ELSE 9000
                END
            )
        )
    ),
    controller_new_cards_per_day = MIN(
        10000,
        COALESCE(
            (
                SELECT decks.new_cards_per_day
                FROM decks
                WHERE decks.id = scheduler_profiles.deck_id
            ),
            20
        )
    ),
    controller_explanation =
        'Migrated policy settings; automatic mode evaluates on the next Today view.';

UPDATE scheduler_profiles
SET daily_time_budget_minutes = NULL
WHERE deck_id = 'default-deck';

CREATE INDEX schedule_states_lifecycle_due
ON schedule_states(lifecycle, due_at_ms, card_id);

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (10, unixepoch('subsec') * 1000);
