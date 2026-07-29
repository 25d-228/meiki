ALTER TABLE decks
ADD COLUMN language_tag TEXT;

ALTER TABLE decks
ADD COLUMN direction TEXT NOT NULL DEFAULT 'auto'
    CHECK (direction IN ('auto', 'ltr', 'rtl'));

ALTER TABLE decks
ADD COLUMN matching_policy TEXT NOT NULL DEFAULT 'strict'
    CHECK (matching_policy IN ('strict', 'forgiving'));

ALTER TABLE clozes
ADD COLUMN matching_policy TEXT
    CHECK (matching_policy IS NULL OR matching_policy IN ('strict', 'forgiving'));

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (3, unixepoch('subsec') * 1000);
