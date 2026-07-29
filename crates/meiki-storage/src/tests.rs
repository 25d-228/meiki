use meiki_domain::{
    Annotation, Card, Cloze, ComparisonResult, Deck, Direction, Grade, LocalizedText,
    MatchingPolicy, MediaKind, MediaReference, ReviewEvent, ScheduleState, SchedulerParameterSet,
    SegmentContent, SemanticSegment, SourceItem, StudySettingsOverride, Tag,
};
use rusqlite::Connection;
use tempfile::tempdir;

use super::{
    AnnotationRepository, CardRepository, ClozeRepository, DEFAULT_DECK_ID, DeckRepository,
    FOUNDATION_MIGRATION, MediaRepository, SAMPLE_CARD_ID, SchedulerParameterSetRepository,
    SourceNoteRepository, Storage, StorageError, StoredSourceNote, TagRepository,
};

fn sample_event(storage: &Storage, id: &str, reviewed_at_ms: i64) -> ReviewEvent {
    let stored = storage.load_study_card(SAMPLE_CARD_ID).unwrap();
    let mut next = stored.schedule.clone();
    next.version += 1;
    next.due_at_ms = reviewed_at_ms + 259_200_000;
    next.interval_seconds = 259_200;
    next.repetitions += 1;
    next.last_review_event_id = Some(id.to_owned());
    ReviewEvent {
        id: id.to_owned(),
        card_id: stored.card.id,
        card_content_version: stored.card.content_version,
        raw_response: " 行きます ".into(),
        normalized_response: "行きます".into(),
        comparison: ComparisonResult::Exact,
        suggested_grade: Grade::Good,
        chosen_grade: Grade::Good,
        reviewed_at_ms,
        scheduler_version: "test-scheduler".into(),
        scheduler_parameter_set_id: None,
        previous_schedule: stored.schedule,
        next_schedule: next,
    }
}

fn deck(id: &str) -> Deck {
    Deck {
        id: id.into(),
        name: "Mixed scripts".into(),
        description: Some("日本語 و فارسی".into()),
        language_tag: None,
        direction: Direction::Auto,
        matching_policy: MatchingPolicy::Strict,
        settings: StudySettingsOverride {
            target_retention_basis_points: Some(9_200),
            new_cards_per_day: Some(12),
            maximum_interval_days: None,
        },
        created_at_ms: 1_000,
        updated_at_ms: 1_000,
    }
}

fn tag(id: &str, name: &str) -> Tag {
    Tag {
        id: id.into(),
        name: name.into(),
        created_at_ms: 1_000,
        updated_at_ms: 1_000,
    }
}

fn annotation(id: &str, label: &str, value: &str, direction: Direction) -> Annotation {
    Annotation {
        id: id.into(),
        label: label.into(),
        value: value.into(),
        language_tag: None,
        direction,
    }
}

fn media(id: &str, kind: MediaKind) -> MediaReference {
    MediaReference {
        id: id.into(),
        content_hash: format!("sha256-{id}"),
        kind,
        media_type: match kind {
            MediaKind::Audio => "audio/ogg",
            MediaKind::Image => "image/png",
        }
        .into(),
        original_file_name: Some("کتاب-図書館.png".into()),
        alt_text: Some("کتابと図書館 👩🏽‍💻".into()),
        language_tag: None,
        direction: Direction::Auto,
        created_at_ms: 1_000,
    }
}

fn japanese_cloze() -> Cloze {
    Cloze {
        id: "cloze-ja".into(),
        source_item_id: "source-mixed".into(),
        answer: "図書館".into(),
        accepted_answers: vec!["としょかん".into()],
        hint: Some(LocalizedText {
            value: "場所".into(),
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
        }),
        language_tag: Some("ja".into()),
        direction: Direction::Auto,
        matching_policy: None,
        annotations: vec![annotation(
            "annotation-ja",
            "読み",
            "としょかん",
            Direction::Auto,
        )],
        explanation: Some(LocalizedText {
            value: "本を読む場所".into(),
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
        }),
        media: vec![media("media-cloze", MediaKind::Audio)],
        created_at_ms: 1_000,
        updated_at_ms: 1_000,
    }
}

fn persian_cloze() -> Cloze {
    Cloze {
        id: "cloze-fa".into(),
        source_item_id: "source-mixed".into(),
        answer: "کتاب".into(),
        accepted_answers: vec!["كتاب".into()],
        hint: Some(LocalizedText {
            value: "چیزی برای خواندن".into(),
            language_tag: Some("fa".into()),
            direction: Direction::RightToLeft,
        }),
        language_tag: Some("fa".into()),
        direction: Direction::RightToLeft,
        matching_policy: Some(MatchingPolicy::Forgiving),
        annotations: vec![Annotation {
            id: "annotation-fa".into(),
            label: "نقش".into(),
            value: "اسم".into(),
            language_tag: Some("fa".into()),
            direction: Direction::RightToLeft,
        }],
        explanation: None,
        media: Vec::new(),
        created_at_ms: 1_000,
        updated_at_ms: 1_000,
    }
}

fn mixed_segments() -> Vec<SemanticSegment> {
    vec![
        SemanticSegment {
            id: "segment-0".into(),
            ordinal: 0,
            content: SegmentContent::Text("昨日、".into()),
        },
        SemanticSegment {
            id: "segment-ja".into(),
            ordinal: 1,
            content: SegmentContent::Cloze {
                cloze_id: "cloze-ja".into(),
                text: "図書館".into(),
            },
        },
        SemanticSegment {
            id: "segment-2".into(),
            ordinal: 2,
            content: SegmentContent::Text("で ".into()),
        },
        SemanticSegment {
            id: "segment-fa".into(),
            ordinal: 3,
            content: SegmentContent::Cloze {
                cloze_id: "cloze-fa".into(),
                text: "کتاب".into(),
            },
        },
        SemanticSegment {
            id: "segment-4".into(),
            ordinal: 4,
            content: SegmentContent::Text(" را خواندم 👩🏽‍💻。".into()),
        },
    ]
}

fn mixed_note() -> StoredSourceNote {
    StoredSourceNote {
        source_item: SourceItem {
            id: "source-mixed".into(),
            deck_id: "deck-mixed".into(),
            segments: mixed_segments(),
            language_tag: None,
            direction: Direction::Auto,
            tags: vec![tag("tag-mixed", "日本語/فارسی")],
            annotations: vec![Annotation {
                id: "annotation-source".into(),
                label: "Register".into(),
                value: "رسمی".into(),
                language_tag: Some("fa".into()),
                direction: Direction::RightToLeft,
            }],
            explanation: Some(LocalizedText {
                value: "混合方向の文 — جملهٔ دوطرفه".into(),
                language_tag: None,
                direction: Direction::Auto,
            }),
            media: vec![media("media-source", MediaKind::Image)],
            created_at_ms: 1_000,
            updated_at_ms: 1_000,
        },
        clozes: vec![japanese_cloze(), persian_cloze()],
    }
}

#[test]
fn sample_data_survives_reopening_the_database() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("collection.db");
    {
        let mut storage = Storage::open(&path).unwrap();
        storage.seed_walking_skeleton(1_000).unwrap();
    }

    let storage = Storage::open(&path).unwrap();
    let restored = storage.load_study_card(SAMPLE_CARD_ID).unwrap();
    assert_eq!(restored.cloze.answer, "行きます");
    assert_eq!(restored.source_item.segments.len(), 2);
    assert_eq!(restored.source_item.deck_id, DEFAULT_DECK_ID);
}

#[test]
fn review_append_projection_and_queue_update_are_atomic() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let parameter_set = SchedulerParameterSet {
        id: "parameters-review".into(),
        engine_version: "test-scheduler".into(),
        parameters: vec![1.0, 2.0],
        created_at_ms: 1_000,
    };
    storage
        .create_scheduler_parameter_set(&parameter_set)
        .unwrap();
    let mut event = sample_event(&storage, "review-1", 10_000);
    event.scheduler_parameter_set_id = Some(parameter_set.id);

    let committed = storage.commit_review(&event).unwrap();
    assert_eq!(committed.version, 1);
    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    assert_eq!(storage.review_events(SAMPLE_CARD_ID).unwrap(), vec![event]);
    let queue_updated_at_ms = storage
        .connection
        .query_row(
            "SELECT queue_updated_at_ms FROM cards WHERE id = ?1",
            [SAMPLE_CARD_ID],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(queue_updated_at_ms, 10_000);

    let mut stale = sample_event(&storage, "review-stale", 20_000);
    stale.previous_schedule.version = 0;
    assert!(matches!(
        storage.commit_review(&stale),
        Err(StorageError::StaleReview)
    ));
    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
}

#[test]
fn review_events_cannot_be_changed_or_deleted_in_place() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let event = sample_event(&storage, "review-1", 10_000);
    storage.commit_review(&event).unwrap();

    let update_error = storage
        .connection
        .execute(
            "UPDATE review_events SET raw_response = 'changed' WHERE id = 'review-1'",
            [],
        )
        .unwrap_err();
    assert!(update_error.to_string().contains("append-only"));

    let delete_error = storage
        .connection
        .execute("DELETE FROM review_events WHERE id = 'review-1'", [])
        .unwrap_err();
    assert!(delete_error.to_string().contains("append-only"));
}

#[test]
fn schedule_projection_rebuilds_from_immutable_events() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let first = sample_event(&storage, "review-z", 10_000);
    storage.commit_review(&first).unwrap();
    let second = sample_event(&storage, "review-a", 10_000);
    let expected = storage.commit_review(&second).unwrap();

    storage
        .connection
        .execute(
            "UPDATE schedule_states
             SET version = 99,
                 due_at_ms = -1,
                 interval_seconds = 0,
                 repetitions = 0,
                 last_review_event_id = NULL
             WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
        )
        .unwrap();

    let rebuilt = storage.rebuild_schedule_projection(SAMPLE_CARD_ID).unwrap();
    assert_eq!(rebuilt, expected);
    assert_eq!(storage.load_schedule(SAMPLE_CARD_ID).unwrap(), expected);
}

#[test]
fn version_one_collection_migrates_to_the_core_model() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("v1.db");
    {
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(FOUNDATION_MIGRATION).unwrap();
        connection
            .execute_batch(
                "INSERT INTO source_items(id, language_tag, direction, created_at_ms)
                 VALUES ('legacy-source', NULL, 'rtl', 1000);
                 INSERT INTO clozes(
                    id, source_item_id, answer, accepted_answers_json
                 ) VALUES ('legacy-cloze', 'legacy-source', 'کتاب', '[]');
                 INSERT INTO semantic_segments(
                    id, source_item_id, ordinal, kind, text, cloze_id
                 ) VALUES (
                    'legacy-segment', 'legacy-source', 0, 'cloze', 'کتاب', 'legacy-cloze'
                 );
                 INSERT INTO cards(id, cloze_id, content_version)
                 VALUES ('legacy-card', 'legacy-cloze', 0);
                 INSERT INTO schedule_states(
                    card_id, version, due_at_ms, interval_seconds, repetitions,
                    last_review_event_id
                 ) VALUES ('legacy-card', 1, 2000, 1, 1, 'legacy-review');
                 INSERT INTO review_events(
                    id,
                    card_id,
                    card_content_version,
                    raw_response,
                    normalized_response,
                    comparison,
                    suggested_grade,
                    chosen_grade,
                    reviewed_at_ms,
                    scheduler_version,
                    previous_schedule_version,
                    previous_due_at_ms,
                    previous_interval_seconds,
                    previous_repetitions,
                    next_schedule_version,
                    next_due_at_ms,
                    next_interval_seconds,
                    next_repetitions
                 ) VALUES (
                    'legacy-review',
                    'legacy-card',
                    0,
                    'کتاب',
                    'کتاب',
                    'exact',
                    'good',
                    'good',
                    1500,
                    'legacy-scheduler',
                    0,
                    1000,
                    0,
                    0,
                    1,
                    2000,
                    1,
                    1
                 );",
            )
            .unwrap();
    }

    let mut storage = Storage::open(&path).unwrap();
    assert_eq!(storage.schema_version().unwrap(), 3);
    let restored = storage.load_study_card("legacy-card").unwrap();
    assert_eq!(restored.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(restored.source_item.direction, Direction::RightToLeft);
    assert_eq!(restored.cloze.answer, "کتاب");
    let baseline = storage
        .rebuildable_baseline_for_test("legacy-card")
        .unwrap();
    assert_eq!(baseline.version, 0);
    assert_eq!(baseline.due_at_ms, 1_000);
    let before = storage.load_schedule("legacy-card").unwrap();
    assert_eq!(
        storage.rebuild_schedule_projection("legacy-card").unwrap(),
        before
    );
}

#[test]
fn backup_restores_content_history_and_projection() {
    let directory = tempdir().unwrap();
    let source_path = directory.path().join("source.db");
    let backup_path = directory.path().join("collection.backup");
    let restored_path = directory.path().join("restored.db");
    let expected_schedule;
    {
        let mut storage = Storage::open(&source_path).unwrap();
        storage.seed_walking_skeleton(1_000).unwrap();
        let event = sample_event(&storage, "review-1", 10_000);
        expected_schedule = storage.commit_review(&event).unwrap();
        storage.backup_to(&backup_path).unwrap();
    }

    let restored = Storage::restore_from_backup(&backup_path, &restored_path).unwrap();
    assert_eq!(
        restored.load_schedule(SAMPLE_CARD_ID).unwrap(),
        expected_schedule
    );
    assert_eq!(restored.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    assert_eq!(
        restored
            .load_study_card(SAMPLE_CARD_ID)
            .unwrap()
            .cloze
            .answer,
        "行きます"
    );
}

#[test]
fn mutable_core_entities_support_create_read_update_delete() {
    let mut storage = Storage::open_in_memory().unwrap();

    let mut deck = deck("deck-crud");
    storage.create_deck(&deck).unwrap();
    assert_eq!(storage.get_deck(&deck.id).unwrap(), deck);
    deck.name = "Updated deck".into();
    deck.updated_at_ms = 2_000;
    storage.update_deck(&deck).unwrap();
    assert_eq!(storage.get_deck(&deck.id).unwrap(), deck);
    storage.delete_deck(&deck.id).unwrap();

    let mut tag = tag("tag-crud", "initial");
    storage.create_tag(&tag).unwrap();
    tag.name = "updated".into();
    tag.updated_at_ms = 2_000;
    storage.update_tag(&tag).unwrap();
    assert_eq!(storage.get_tag(&tag.id).unwrap(), tag);
    storage.delete_tag(&tag.id).unwrap();

    let mut annotation = annotation("annotation-crud", "Type", "Initial", Direction::LeftToRight);
    storage.create_annotation(&annotation).unwrap();
    annotation.value = "Updated".into();
    storage.update_annotation(&annotation).unwrap();
    assert_eq!(storage.get_annotation(&annotation.id).unwrap(), annotation);
    storage.delete_annotation(&annotation.id).unwrap();

    let mut media = media("media-crud", MediaKind::Audio);
    storage.create_media_reference(&media).unwrap();
    media.alt_text = Some("Updated description".into());
    storage.update_media_reference(&media).unwrap();
    assert_eq!(storage.get_media_reference(&media.id).unwrap(), media);
    storage.delete_media_reference(&media.id).unwrap();

    let mut parameter_set = SchedulerParameterSet {
        id: "parameters-crud".into(),
        engine_version: "fsrs-7".into(),
        parameters: vec![0.1, 1.25, 3.5],
        created_at_ms: 1_000,
    };
    storage
        .create_scheduler_parameter_set(&parameter_set)
        .unwrap();
    parameter_set.parameters.push(8.0);
    storage
        .update_scheduler_parameter_set(&parameter_set)
        .unwrap();
    assert_eq!(
        storage
            .get_scheduler_parameter_set(&parameter_set.id)
            .unwrap(),
        parameter_set
    );
    storage
        .delete_scheduler_parameter_set(&parameter_set.id)
        .unwrap();
}

#[test]
fn multilingual_aggregate_round_trips_and_cloze_ids_survive_surrounding_edits() {
    let mut storage = Storage::open_in_memory().unwrap();
    let deck = deck("deck-mixed");
    storage.create_deck(&deck).unwrap();
    let mut note = mixed_note();
    storage.create_source_note(&note).unwrap();
    assert_eq!(storage.get_source_note(&note.source_item.id).unwrap(), note);

    for (index, cloze) in note.clozes.iter().enumerate() {
        let mut card = Card {
            id: format!("card-{index}"),
            cloze_id: cloze.id.clone(),
            content_version: 0,
            settings: StudySettingsOverride::default(),
            created_at_ms: 1_000,
            updated_at_ms: 1_000,
        };
        let schedule = ScheduleState {
            card_id: card.id.clone(),
            version: 0,
            due_at_ms: 1_000,
            interval_seconds: 0,
            repetitions: 0,
            last_review_event_id: None,
        };
        storage.create_card(&card, &schedule).unwrap();
        assert_eq!(storage.get_card(&card.id).unwrap(), card);
        card.content_version = 1;
        card.settings.maximum_interval_days = Some(2_000);
        card.updated_at_ms = 2_000;
        storage.update_card(&card).unwrap();
        assert_eq!(storage.get_card(&card.id).unwrap(), card);
    }
    let mut moved_card = storage.get_card("card-0").unwrap();
    moved_card.cloze_id = "cloze-fa".into();
    assert!(matches!(
        storage.update_card(&moved_card),
        Err(StorageError::InvalidAggregate(_))
    ));

    note.source_item.segments[0].content = SegmentContent::Text("先週、".into());
    note.source_item.segments[4].content = SegmentContent::Text(" را با دقت خواندم 👩🏽‍💻。".into());
    note.source_item.updated_at_ms = 2_000;
    storage.update_source_note(&note).unwrap();

    let restored = storage.get_source_note(&note.source_item.id).unwrap();
    assert_eq!(restored, note);
    assert_eq!(
        restored
            .clozes
            .iter()
            .map(|cloze| cloze.id.as_str())
            .collect::<Vec<_>>(),
        vec!["cloze-ja", "cloze-fa"]
    );
    assert_eq!(storage.get_cloze("cloze-fa").unwrap().answer, "کتاب");

    let mut updated_cloze = storage.get_cloze("cloze-fa").unwrap();
    updated_cloze.answer = "دفتر".into();
    updated_cloze.hint = Some(LocalizedText {
        value: "برای مطالعه".into(),
        language_tag: Some("fa".into()),
        direction: Direction::RightToLeft,
    });
    updated_cloze.updated_at_ms = 3_000;
    storage.update_cloze(&updated_cloze).unwrap();
    assert_eq!(storage.get_cloze("cloze-fa").unwrap(), updated_cloze);
    let updated_note = storage.get_source_note(&note.source_item.id).unwrap();
    assert!(matches!(
        &updated_note.source_item.segments[3].content,
        SegmentContent::Cloze { cloze_id, text }
            if cloze_id == "cloze-fa" && text == "دفتر"
    ));

    storage.delete_card("card-0").unwrap();
    assert!(matches!(
        storage.get_card("card-0"),
        Err(StorageError::EntityNotFound { entity: "card", .. })
    ));
    storage.delete_source_note(&note.source_item.id).unwrap();
    assert!(matches!(
        storage.get_source_note(&note.source_item.id),
        Err(StorageError::EntityNotFound {
            entity: "source note",
            ..
        })
    ));
    storage.delete_tag("tag-mixed").unwrap();
    storage.delete_media_reference("media-source").unwrap();
    storage.delete_media_reference("media-cloze").unwrap();
    storage.delete_deck(&deck.id).unwrap();
}

impl Storage {
    fn rebuildable_baseline_for_test(&self, card_id: &str) -> Result<ScheduleState, StorageError> {
        super::repository::load_schedule_row(&self.connection, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))
    }
}
