ALTER TABLE source_items
ADD COLUMN deleted_at_ms INTEGER;

CREATE INDEX source_items_library_order
ON source_items(deleted_at_ms, updated_at_ms DESC, id);

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (7, unixepoch('subsec') * 1000);
