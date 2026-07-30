ALTER TABLE schedule_states
ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'unseen'
    CHECK (lifecycle IN ('unseen', 'introduced'));

ALTER TABLE schedule_baselines
ADD COLUMN lifecycle TEXT NOT NULL DEFAULT 'unseen'
    CHECK (lifecycle IN ('unseen', 'introduced'));

ALTER TABLE review_events
ADD COLUMN previous_lifecycle TEXT NOT NULL DEFAULT 'unseen'
    CHECK (previous_lifecycle IN ('unseen', 'introduced'));

ALTER TABLE review_events
ADD COLUMN next_lifecycle TEXT NOT NULL DEFAULT 'unseen'
    CHECK (next_lifecycle IN ('unseen', 'introduced'));

DROP TRIGGER review_events_are_append_only_update;
DROP TRIGGER review_events_are_append_only_delete;

UPDATE schedule_baselines
SET lifecycle = 'introduced'
WHERE repetitions > 0
   OR stability_milliseconds > 0
   OR difficulty_millipoints > 0
   OR last_reviewed_at_ms IS NOT NULL;

UPDATE review_events AS current_event
SET previous_lifecycle = CASE
        WHEN current_event.previous_repetitions > 0
          OR current_event.previous_stability_milliseconds > 0
          OR current_event.previous_difficulty_millipoints > 0
          OR current_event.previous_last_reviewed_at_ms IS NOT NULL
          OR (
              SELECT COALESCE(SUM(
                  CASE prior_event.event_kind
                      WHEN 'review' THEN 1
                      WHEN 'undo' THEN -1
                  END
              ), 0)
              FROM review_events AS prior_event
              WHERE prior_event.card_id = current_event.card_id
                AND prior_event.previous_schedule_version
                    < current_event.previous_schedule_version
          ) > 0
        THEN 'introduced'
        ELSE 'unseen'
    END,
    next_lifecycle = CASE
        WHEN current_event.next_repetitions > 0
          OR current_event.next_stability_milliseconds > 0
          OR current_event.next_difficulty_millipoints > 0
          OR current_event.next_last_reviewed_at_ms IS NOT NULL
          OR (
              SELECT COALESCE(SUM(
                  CASE prior_event.event_kind
                      WHEN 'review' THEN 1
                      WHEN 'undo' THEN -1
                  END
              ), 0)
              FROM review_events AS prior_event
              WHERE prior_event.card_id = current_event.card_id
                AND prior_event.previous_schedule_version
                    <= current_event.previous_schedule_version
          ) > 0
        THEN 'introduced'
        ELSE 'unseen'
    END;

UPDATE schedule_states
SET lifecycle = 'introduced'
WHERE repetitions > 0
   OR stability_milliseconds > 0
   OR difficulty_millipoints > 0
   OR last_reviewed_at_ms IS NOT NULL
   OR (
       SELECT COALESCE(SUM(
           CASE review_events.event_kind
               WHEN 'review' THEN 1
               WHEN 'undo' THEN -1
           END
       ), 0)
       FROM review_events
       WHERE review_events.card_id = schedule_states.card_id
   ) > 0;

CREATE TRIGGER review_events_are_append_only_update
BEFORE UPDATE ON review_events
BEGIN
    SELECT RAISE(ABORT, 'review events are append-only');
END;

CREATE TRIGGER review_events_are_append_only_delete
BEFORE DELETE ON review_events
BEGIN
    SELECT RAISE(ABORT, 'review events are append-only');
END;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (8, unixepoch('subsec') * 1000);
