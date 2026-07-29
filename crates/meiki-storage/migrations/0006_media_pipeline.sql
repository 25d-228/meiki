ALTER TABLE media_references
ADD COLUMN role TEXT NOT NULL DEFAULT 'answer_audio'
    CHECK (role IN ('prompt_audio', 'answer_audio', 'reveal_image'));

ALTER TABLE media_references
ADD COLUMN byte_size INTEGER NOT NULL DEFAULT 0
    CHECK (byte_size >= 0);

ALTER TABLE media_references
ADD COLUMN width INTEGER
    CHECK (width IS NULL OR width > 0);

ALTER TABLE media_references
ADD COLUMN height INTEGER
    CHECK (height IS NULL OR height > 0);

ALTER TABLE media_references
ADD COLUMN duration_ms INTEGER
    CHECK (duration_ms IS NULL OR duration_ms >= 0);

UPDATE media_references
SET role = CASE
    WHEN kind = 'image' THEN 'reveal_image'
    WHEN EXISTS (
        SELECT 1
        FROM source_item_media
        WHERE source_item_media.media_reference_id = media_references.id
    ) THEN 'prompt_audio'
    ELSE 'answer_audio'
END;

CREATE INDEX media_references_content_hash
ON media_references(content_hash);

CREATE TRIGGER media_references_role_matches_kind_insert
BEFORE INSERT ON media_references
WHEN
    (NEW.kind = 'image' AND NEW.role != 'reveal_image')
    OR
    (NEW.kind = 'audio' AND NEW.role = 'reveal_image')
BEGIN
    SELECT RAISE(ABORT, 'media role does not match media kind');
END;

CREATE TRIGGER media_references_role_matches_kind_update
BEFORE UPDATE OF kind, role ON media_references
WHEN
    (NEW.kind = 'image' AND NEW.role != 'reveal_image')
    OR
    (NEW.kind = 'audio' AND NEW.role = 'reveal_image')
BEGIN
    SELECT RAISE(ABORT, 'media role does not match media kind');
END;

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (6, unixepoch('subsec') * 1000);
