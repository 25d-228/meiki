ALTER TABLE cards
ADD COLUMN suspended INTEGER NOT NULL DEFAULT 0
    CHECK (suspended IN (0, 1));

ALTER TABLE review_events
ADD COLUMN event_kind TEXT NOT NULL DEFAULT 'review'
    CHECK (event_kind IN ('review', 'undo'));

ALTER TABLE review_events
ADD COLUMN undoes_review_event_id TEXT
    REFERENCES review_events(id) ON DELETE RESTRICT;

ALTER TABLE review_events
ADD COLUMN response_duration_ms INTEGER NOT NULL DEFAULT 0
    CHECK (response_duration_ms >= 0);

ALTER TABLE review_events
ADD COLUMN grade_overridden INTEGER NOT NULL DEFAULT 0
    CHECK (grade_overridden IN (0, 1));

CREATE UNIQUE INDEX review_events_one_undo
ON review_events(undoes_review_event_id)
WHERE undoes_review_event_id IS NOT NULL;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (5, unixepoch('subsec') * 1000);
