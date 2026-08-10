CREATE INDEX semantic_segments_cloze
ON semantic_segments(cloze_id);

CREATE INDEX source_item_tags_tag
ON source_item_tags(tag_id);

CREATE INDEX source_item_annotations_annotation
ON source_item_annotations(annotation_id);

CREATE INDEX cloze_annotations_annotation
ON cloze_annotations(annotation_id);

CREATE INDEX source_item_media_reference
ON source_item_media(media_reference_id);

CREATE INDEX cloze_media_reference
ON cloze_media(media_reference_id);

INSERT INTO schema_migrations(version, applied_at_ms)
VALUES (13, unixepoch('subsec') * 1000);
