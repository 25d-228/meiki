CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE source_items (
    id TEXT PRIMARY KEY,
    language_tag TEXT,
    direction TEXT NOT NULL CHECK (direction IN ('auto', 'ltr', 'rtl')),
    created_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE clozes (
    id TEXT PRIMARY KEY,
    source_item_id TEXT NOT NULL REFERENCES source_items(id) ON DELETE CASCADE,
    answer TEXT NOT NULL,
    accepted_answers_json TEXT NOT NULL
) STRICT;

CREATE TABLE semantic_segments (
    id TEXT PRIMARY KEY,
    source_item_id TEXT NOT NULL REFERENCES source_items(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    kind TEXT NOT NULL CHECK (kind IN ('text', 'cloze')),
    text TEXT NOT NULL,
    cloze_id TEXT REFERENCES clozes(id) ON DELETE RESTRICT,
    UNIQUE (source_item_id, ordinal),
    CHECK (
        (kind = 'text' AND cloze_id IS NULL)
        OR (kind = 'cloze' AND cloze_id IS NOT NULL)
    )
) STRICT;

CREATE TABLE cards (
    id TEXT PRIMARY KEY,
    cloze_id TEXT NOT NULL UNIQUE REFERENCES clozes(id) ON DELETE RESTRICT,
    content_version INTEGER NOT NULL CHECK (content_version >= 0)
) STRICT;

CREATE TABLE schedule_states (
    card_id TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version >= 0),
    due_at_ms INTEGER NOT NULL,
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds >= 0),
    repetitions INTEGER NOT NULL CHECK (repetitions >= 0),
    last_review_event_id TEXT
) STRICT;

CREATE TABLE review_events (
    id TEXT PRIMARY KEY,
    card_id TEXT NOT NULL REFERENCES cards(id) ON DELETE RESTRICT,
    card_content_version INTEGER NOT NULL,
    raw_response TEXT NOT NULL,
    normalized_response TEXT NOT NULL,
    comparison TEXT NOT NULL CHECK (
        comparison IN ('exact', 'accepted_variant', 'near_match', 'incorrect', 'empty')
    ),
    suggested_grade TEXT NOT NULL CHECK (
        suggested_grade IN ('again', 'hard', 'good', 'easy')
    ),
    chosen_grade TEXT NOT NULL CHECK (
        chosen_grade IN ('again', 'hard', 'good', 'easy')
    ),
    reviewed_at_ms INTEGER NOT NULL,
    scheduler_version TEXT NOT NULL,
    previous_schedule_version INTEGER NOT NULL,
    previous_due_at_ms INTEGER NOT NULL,
    previous_interval_seconds INTEGER NOT NULL,
    previous_repetitions INTEGER NOT NULL,
    next_schedule_version INTEGER NOT NULL,
    next_due_at_ms INTEGER NOT NULL,
    next_interval_seconds INTEGER NOT NULL,
    next_repetitions INTEGER NOT NULL
) STRICT;

CREATE INDEX review_events_card_time
ON review_events(card_id, reviewed_at_ms, id);

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
VALUES (1, unixepoch('subsec') * 1000);
