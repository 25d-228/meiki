CREATE TABLE decks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    target_retention_basis_points INTEGER
        CHECK (
            target_retention_basis_points IS NULL
            OR target_retention_basis_points BETWEEN 1 AND 10000
        ),
    new_cards_per_day INTEGER CHECK (new_cards_per_day IS NULL OR new_cards_per_day >= 0),
    maximum_interval_days INTEGER
        CHECK (maximum_interval_days IS NULL OR maximum_interval_days > 0),
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO decks(id, name, created_at_ms, updated_at_ms)
VALUES ('default-deck', 'Default', 0, 0);

ALTER TABLE source_items
ADD COLUMN deck_id TEXT REFERENCES decks(id) ON DELETE RESTRICT;

ALTER TABLE source_items
ADD COLUMN explanation TEXT;

ALTER TABLE source_items
ADD COLUMN explanation_language_tag TEXT;

ALTER TABLE source_items
ADD COLUMN explanation_direction TEXT
    CHECK (explanation_direction IS NULL OR explanation_direction IN ('auto', 'ltr', 'rtl'));

ALTER TABLE source_items
ADD COLUMN updated_at_ms INTEGER NOT NULL DEFAULT 0;

UPDATE source_items
SET deck_id = 'default-deck', updated_at_ms = created_at_ms
WHERE deck_id IS NULL;

ALTER TABLE clozes
ADD COLUMN hint TEXT;

ALTER TABLE clozes
ADD COLUMN hint_language_tag TEXT;

ALTER TABLE clozes
ADD COLUMN hint_direction TEXT
    CHECK (hint_direction IS NULL OR hint_direction IN ('auto', 'ltr', 'rtl'));

ALTER TABLE clozes
ADD COLUMN language_tag TEXT;

ALTER TABLE clozes
ADD COLUMN direction TEXT NOT NULL DEFAULT 'auto'
    CHECK (direction IN ('auto', 'ltr', 'rtl'));

ALTER TABLE clozes
ADD COLUMN explanation TEXT;

ALTER TABLE clozes
ADD COLUMN explanation_language_tag TEXT;

ALTER TABLE clozes
ADD COLUMN explanation_direction TEXT
    CHECK (explanation_direction IS NULL OR explanation_direction IN ('auto', 'ltr', 'rtl'));

ALTER TABLE clozes
ADD COLUMN created_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE clozes
ADD COLUMN updated_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE cards
ADD COLUMN target_retention_basis_points INTEGER
    CHECK (
        target_retention_basis_points IS NULL
        OR target_retention_basis_points BETWEEN 1 AND 10000
    );

ALTER TABLE cards
ADD COLUMN new_cards_per_day INTEGER
    CHECK (new_cards_per_day IS NULL OR new_cards_per_day >= 0);

ALTER TABLE cards
ADD COLUMN maximum_interval_days INTEGER
    CHECK (maximum_interval_days IS NULL OR maximum_interval_days > 0);

ALTER TABLE cards
ADD COLUMN created_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE cards
ADD COLUMN updated_at_ms INTEGER NOT NULL DEFAULT 0;

ALTER TABLE cards
ADD COLUMN queue_updated_at_ms INTEGER NOT NULL DEFAULT 0;

CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE source_item_tags (
    source_item_id TEXT NOT NULL REFERENCES source_items(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (source_item_id, tag_id),
    UNIQUE (source_item_id, ordinal)
) STRICT;

CREATE TABLE annotations (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    value TEXT NOT NULL,
    language_tag TEXT,
    direction TEXT NOT NULL CHECK (direction IN ('auto', 'ltr', 'rtl'))
) STRICT;

CREATE TABLE source_item_annotations (
    source_item_id TEXT NOT NULL REFERENCES source_items(id) ON DELETE CASCADE,
    annotation_id TEXT NOT NULL REFERENCES annotations(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (source_item_id, annotation_id),
    UNIQUE (source_item_id, ordinal)
) STRICT;

CREATE TABLE cloze_annotations (
    cloze_id TEXT NOT NULL REFERENCES clozes(id) ON DELETE CASCADE,
    annotation_id TEXT NOT NULL REFERENCES annotations(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (cloze_id, annotation_id),
    UNIQUE (cloze_id, ordinal)
) STRICT;

CREATE TABLE media_references (
    id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('audio', 'image')),
    media_type TEXT NOT NULL,
    original_file_name TEXT,
    alt_text TEXT,
    language_tag TEXT,
    direction TEXT NOT NULL CHECK (direction IN ('auto', 'ltr', 'rtl')),
    created_at_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE source_item_media (
    source_item_id TEXT NOT NULL REFERENCES source_items(id) ON DELETE CASCADE,
    media_reference_id TEXT NOT NULL REFERENCES media_references(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (source_item_id, media_reference_id),
    UNIQUE (source_item_id, ordinal)
) STRICT;

CREATE TABLE cloze_media (
    cloze_id TEXT NOT NULL REFERENCES clozes(id) ON DELETE CASCADE,
    media_reference_id TEXT NOT NULL REFERENCES media_references(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (cloze_id, media_reference_id),
    UNIQUE (cloze_id, ordinal)
) STRICT;

CREATE TABLE scheduler_parameter_sets (
    id TEXT PRIMARY KEY,
    engine_version TEXT NOT NULL,
    parameters_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL
) STRICT;

ALTER TABLE review_events
ADD COLUMN scheduler_parameter_set_id TEXT
    REFERENCES scheduler_parameter_sets(id) ON DELETE RESTRICT;

CREATE TABLE schedule_baselines (
    card_id TEXT PRIMARY KEY REFERENCES cards(id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version >= 0),
    due_at_ms INTEGER NOT NULL,
    interval_seconds INTEGER NOT NULL CHECK (interval_seconds >= 0),
    repetitions INTEGER NOT NULL CHECK (repetitions >= 0),
    last_review_event_id TEXT
) STRICT;

INSERT INTO schedule_baselines(
    card_id,
    version,
    due_at_ms,
    interval_seconds,
    repetitions,
    last_review_event_id
)
SELECT
    schedule_states.card_id,
    COALESCE(
        (
            SELECT review_events.previous_schedule_version
            FROM review_events
            WHERE review_events.card_id = schedule_states.card_id
            ORDER BY
                review_events.previous_schedule_version,
                review_events.reviewed_at_ms,
                review_events.id
            LIMIT 1
        ),
        schedule_states.version
    ),
    COALESCE(
        (
            SELECT review_events.previous_due_at_ms
            FROM review_events
            WHERE review_events.card_id = schedule_states.card_id
            ORDER BY
                review_events.previous_schedule_version,
                review_events.reviewed_at_ms,
                review_events.id
            LIMIT 1
        ),
        schedule_states.due_at_ms
    ),
    COALESCE(
        (
            SELECT review_events.previous_interval_seconds
            FROM review_events
            WHERE review_events.card_id = schedule_states.card_id
            ORDER BY
                review_events.previous_schedule_version,
                review_events.reviewed_at_ms,
                review_events.id
            LIMIT 1
        ),
        schedule_states.interval_seconds
    ),
    COALESCE(
        (
            SELECT review_events.previous_repetitions
            FROM review_events
            WHERE review_events.card_id = schedule_states.card_id
            ORDER BY
                review_events.previous_schedule_version,
                review_events.reviewed_at_ms,
                review_events.id
            LIMIT 1
        ),
        schedule_states.repetitions
    ),
    NULL
FROM schedule_states;

CREATE INDEX source_items_deck
ON source_items(deck_id);

CREATE INDEX clozes_source
ON clozes(source_item_id);

CREATE INDEX review_events_card_version
ON review_events(card_id, previous_schedule_version);

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (2, unixepoch('subsec') * 1000);
