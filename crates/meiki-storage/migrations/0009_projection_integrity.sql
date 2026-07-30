CREATE TABLE schedule_projection_repairs (
    schema_version INTEGER PRIMARY KEY,
    repaired_cards INTEGER NOT NULL CHECK (repaired_cards >= 0),
    repaired_at_ms INTEGER NOT NULL
) STRICT;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (9, unixepoch('subsec') * 1000);
