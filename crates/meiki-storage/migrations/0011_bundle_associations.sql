CREATE TABLE bundle_installations (
    language_tag TEXT PRIMARY KEY CHECK (TRIM(language_tag) <> ''),
    installed_at_ms INTEGER NOT NULL CHECK (installed_at_ms >= 0)
) STRICT;

CREATE TABLE bundle_decks (
    language_tag TEXT NOT NULL
        REFERENCES bundle_installations(language_tag) ON DELETE CASCADE,
    deck_id TEXT NOT NULL UNIQUE
        REFERENCES decks(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (language_tag, deck_id),
    UNIQUE (language_tag, ordinal)
) STRICT;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (11, unixepoch('subsec') * 1000);
