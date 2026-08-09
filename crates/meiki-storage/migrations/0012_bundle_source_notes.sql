CREATE TABLE bundle_source_notes (
    source_item_id TEXT PRIMARY KEY
        REFERENCES source_items(id) ON DELETE CASCADE,
    language_tag TEXT NOT NULL
        REFERENCES bundle_installations(language_tag) ON DELETE CASCADE,
    deck_id TEXT NOT NULL
) STRICT;

CREATE INDEX bundle_source_notes_stage
ON bundle_source_notes(language_tag, deck_id);

INSERT INTO bundle_source_notes(source_item_id, language_tag, deck_id)
SELECT source_items.id, bundle_decks.language_tag, bundle_decks.deck_id
FROM bundle_decks
JOIN bundle_installations
  ON bundle_installations.language_tag = bundle_decks.language_tag
JOIN decks ON decks.id = bundle_decks.deck_id
JOIN source_items ON source_items.deck_id = bundle_decks.deck_id
WHERE source_items.created_at_ms = decks.created_at_ms;

CREATE TRIGGER bundle_source_notes_leave_stage
AFTER UPDATE OF deck_id ON source_items
WHEN OLD.deck_id != NEW.deck_id
BEGIN
    DELETE FROM bundle_source_notes WHERE source_item_id = NEW.id;
END;

DROP TRIGGER review_events_are_append_only_delete;

CREATE TRIGGER review_events_are_append_only_delete
BEFORE DELETE ON review_events
WHEN NOT EXISTS (
    SELECT 1
    FROM bundle_source_notes
    JOIN clozes
      ON clozes.source_item_id = bundle_source_notes.source_item_id
    JOIN cards ON cards.cloze_id = clozes.id
    WHERE cards.id = OLD.card_id
)
BEGIN
    SELECT RAISE(ABORT, 'review events are append-only');
END;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (12, unixepoch('subsec') * 1000);
