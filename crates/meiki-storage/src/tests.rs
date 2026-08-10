use meiki_domain::{
    Annotation, Card, CardLifecycle, Cloze, ComparisonResult, Deck, Direction, Grade,
    LocalizedText, MatchingPolicy, MediaKind, MediaReference, MediaRole, ReviewEvent,
    ReviewEventKind, ScheduleState, SchedulerParameterSet, SchedulingMode, SegmentContent,
    SemanticSegment, SourceItem, StudySettingsOverride, Tag,
};
use rusqlite::Connection;
use tempfile::tempdir;

use super::{
    AUTHORING_DEFAULTS_MIGRATION, AnnotationRepository, CARD_LIFECYCLE_MIGRATION,
    CORE_MODEL_MIGRATION, CardRepository, ClozeRepository, DEFAULT_DECK_ID, DeckRepository,
    FOUNDATION_MIGRATION, FSRS7_SCHEDULER_MIGRATION, LIBRARY_MIGRATION, MEDIA_PIPELINE_MIGRATION,
    MediaRepository, PROJECTION_INTEGRITY_MIGRATION, PristineBundleImport, PristineDeckCard,
    PristineDeckImport, PristineDeckNote, SAMPLE_CARD_ID, SAMPLE_CLOZE_ID, SAMPLE_SOURCE_ID,
    STUDY_SESSION_MIGRATION, SchedulerParameterSetRepository, SchedulerProfileRepository,
    SourceNoteRepository, Storage, StorageError, StoredSourceNote, TagRepository,
};

fn sample_event(storage: &Storage, id: &str, reviewed_at_ms: i64) -> ReviewEvent {
    event_for_card(storage, SAMPLE_CARD_ID, id, reviewed_at_ms)
}

fn event_for_card(storage: &Storage, card_id: &str, id: &str, reviewed_at_ms: i64) -> ReviewEvent {
    let stored = storage.load_study_card(card_id).unwrap();
    let mut next = stored.schedule.clone();
    next.version += 1;
    next.lifecycle = CardLifecycle::Introduced;
    next.due_at_ms = reviewed_at_ms + 259_200_000;
    next.ideal_due_at_ms = next.due_at_ms;
    next.interval_milliseconds = 259_200_000;
    next.interval_seconds = 259_200;
    next.repetitions += 1;
    next.stability_milliseconds = 259_200_000;
    next.difficulty_millipoints = 5_000;
    next.last_reviewed_at_ms = Some(reviewed_at_ms);
    next.last_review_event_id = Some(id.to_owned());
    ReviewEvent {
        id: id.to_owned(),
        card_id: stored.card.id,
        card_content_version: stored.card.content_version,
        kind: ReviewEventKind::Review,
        undoes_review_event_id: None,
        raw_response: " 行きます ".into(),
        normalized_response: "行きます".into(),
        comparison: ComparisonResult::Exact,
        suggested_grade: Grade::Good,
        chosen_grade: Grade::Good,
        grade_overridden: false,
        response_duration_ms: 850,
        reviewed_at_ms,
        scheduler_version: "test-scheduler".into(),
        scheduler_parameter_set_id: None,
        target_retention_basis_points: 9_000,
        previous_schedule: stored.schedule,
        next_schedule: next,
    }
}

#[derive(Debug)]
struct LifecycleModel {
    active_reviews: usize,
    event_count: usize,
    suspended: bool,
    trashed: bool,
    content_version: u64,
}

fn assert_lifecycle_model(storage: &Storage, model: &LifecycleModel) {
    let projection_count = storage
        .connection
        .query_row(
            "SELECT COUNT(*) FROM schedule_states WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
            |row| row.get::<_, u64>(0),
        )
        .unwrap();
    let baseline_count = storage
        .connection
        .query_row(
            "SELECT COUNT(*) FROM schedule_baselines WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
            |row| row.get::<_, u64>(0),
        )
        .unwrap();
    assert_eq!(projection_count, 1);
    assert_eq!(baseline_count, 1);

    let events = storage.review_events(SAMPLE_CARD_ID).unwrap();
    assert_eq!(events.len(), model.event_count);
    assert_eq!(
        events
            .iter()
            .map(|event| event.id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        events.len()
    );
    for (index, event) in events.iter().enumerate() {
        assert_eq!(
            event.previous_schedule.version,
            u64::try_from(index).unwrap()
        );
        assert_eq!(
            event.next_schedule.version,
            u64::try_from(index + 1).unwrap()
        );
    }

    let active = storage.active_review_events(SAMPLE_CARD_ID).unwrap();
    assert_eq!(active.len(), model.active_reviews);
    let current = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
    let expected = events.last().map_or_else(
        || storage.load_schedule_baseline(SAMPLE_CARD_ID).unwrap(),
        |event| event.next_schedule.clone(),
    );
    assert_eq!(current, expected);
    assert_eq!(
        current.lifecycle,
        if model.active_reviews == 0 {
            CardLifecycle::Unseen
        } else {
            CardLifecycle::Introduced
        }
    );
    assert_eq!(
        storage.get_card(SAMPLE_CARD_ID).unwrap().content_version,
        model.content_version
    );
    assert_eq!(
        storage.get_card(SAMPLE_CARD_ID).unwrap().suspended,
        model.suspended
    );
    let stored_note = storage
        .library_notes()
        .unwrap()
        .into_iter()
        .find(|stored| stored.note.source_item.id == SAMPLE_SOURCE_ID)
        .unwrap();
    assert_eq!(stored_note.deleted_at_ms.is_some(), model.trashed);

    let workload = storage
        .scheduling_workload(DEFAULT_DECK_ID, 1_000_000_000, 3_419_200_000)
        .unwrap();
    if model.suspended || model.trashed {
        assert_eq!(workload.unseen_cards, 0);
        assert_eq!(workload.due_cards_now, 0);
        assert_eq!(workload.forecast_review_occurrences, 0);
    } else {
        assert!(
            workload.unseen_cards > 0
                || workload.due_cards_now > 0
                || workload.forecast_review_occurrences > 0
        );
    }
    assert!(
        storage
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
}

fn migration_backup_schema_version(directory: &std::path::Path, prefix: &str) -> u32 {
    let backup = std::fs::read_dir(directory.join("backups"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .into_iter()
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(prefix))
        })
        .unwrap()
        .path();
    Connection::open(backup)
        .unwrap()
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

fn open_version_eight_fixture(path: &std::path::Path) -> Storage {
    let connection = Connection::open(path).unwrap();
    for migration in [
        FOUNDATION_MIGRATION,
        CORE_MODEL_MIGRATION,
        AUTHORING_DEFAULTS_MIGRATION,
        FSRS7_SCHEDULER_MIGRATION,
        STUDY_SESSION_MIGRATION,
        MEDIA_PIPELINE_MIGRATION,
        LIBRARY_MIGRATION,
        CARD_LIFECYCLE_MIGRATION,
    ] {
        connection.execute_batch(migration).unwrap();
    }
    Storage { connection }
}

fn open_version_nine_fixture(path: &std::path::Path) -> Storage {
    let storage = open_version_eight_fixture(path);
    storage
        .connection
        .execute_batch(PROJECTION_INTEGRITY_MIGRATION)
        .unwrap();
    storage
}

#[test]
fn opening_a_clean_collection_is_idempotent_and_creates_no_learning_data() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("collection.db");
    drop(Storage::open(&path).unwrap());
    let storage = Storage::open(&path).unwrap();

    assert!(!storage.has_learning_material().unwrap());
    let count = |table: &str| -> i64 {
        storage
            .connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    assert_eq!(count("decks"), 1);
    assert_eq!(count("scheduler_profiles"), 1);
    assert_eq!(count("source_items"), 0);
    assert_eq!(count("clozes"), 0);
    assert_eq!(count("cards"), 0);
    assert_eq!(count("review_events"), 0);
}

#[test]
fn large_performance_fixture_preserves_production_history_invariants() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .seed_large_performance_fixture(30, 1_000_000)
        .unwrap();

    assert_eq!(storage.library_notes().unwrap().len(), 30);
    assert_eq!(
        storage
            .connection
            .query_row("SELECT COUNT(*) FROM review_events", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap(),
        20
    );
    let integrity = storage.check_collection_schedule_integrity().unwrap();
    assert_eq!(integrity.checked_cards, 30);
    assert!(integrity.is_valid());
    let workload = storage
        .scheduling_workload(DEFAULT_DECK_ID, 1_000_000, 31 * 86_400_000)
        .unwrap();
    assert!(workload.due_cards_now > 0);
    assert!(workload.unseen_cards > 0);
    assert!(workload.review_count > 0);
}

#[test]
fn parameter_adoption_is_atomic_and_prospective() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let schedule_before = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
    let history_before = storage.review_count(SAMPLE_CARD_ID).unwrap();
    let default_profile = storage.get_scheduler_profile(DEFAULT_DECK_ID).unwrap();
    let mut parameters = storage
        .get_scheduler_parameter_set(&default_profile.active_parameter_set_id)
        .unwrap()
        .parameters;
    parameters[7] += 0.1;

    let personalized = SchedulerParameterSet {
        id: "fsrs7-personal-test".into(),
        engine_version: "fsrs-7".into(),
        parameters,
        created_at_ms: 2_000,
    };
    let adopted = storage
        .adopt_scheduler_parameter_set(DEFAULT_DECK_ID, &personalized, 2_000)
        .unwrap();
    assert_eq!(adopted.active_parameter_set_id, personalized.id);
    assert_eq!(
        storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
        schedule_before
    );
    assert_eq!(
        storage.review_count(SAMPLE_CARD_ID).unwrap(),
        history_before
    );
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
        role: match kind {
            MediaKind::Audio => MediaRole::AnswerAudio,
            MediaKind::Image => MediaRole::RevealImage,
        },
        media_type: match kind {
            MediaKind::Audio => "audio/ogg",
            MediaKind::Image => "image/png",
        }
        .into(),
        byte_size: 1_024,
        original_file_name: Some("کتاب-図書館.png".into()),
        alt_text: Some("کتابと図書館 👩🏽‍💻".into()),
        width: (kind == MediaKind::Image).then_some(640),
        height: (kind == MediaKind::Image).then_some(480),
        duration_ms: (kind == MediaKind::Audio).then_some(2_500),
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

fn pristine_deck_import() -> PristineDeckImport {
    let deck = Deck {
        settings: StudySettingsOverride::default(),
        ..deck("deck-mixed")
    };
    let note = mixed_note();
    let cards = note
        .clozes
        .iter()
        .enumerate()
        .map(|(index, cloze)| {
            let card = Card {
                id: format!("pristine-card-{index}"),
                cloze_id: cloze.id.clone(),
                content_version: 0,
                suspended: false,
                created_at_ms: 2_000,
                updated_at_ms: 2_000,
            };
            PristineDeckCard {
                initial_schedule: ScheduleState {
                    card_id: card.id.clone(),
                    version: 0,
                    lifecycle: CardLifecycle::Unseen,
                    due_at_ms: 2_000,
                    ideal_due_at_ms: 2_000,
                    interval_milliseconds: 0,
                    interval_seconds: 0,
                    repetitions: 0,
                    stability_milliseconds: 0,
                    difficulty_millipoints: 0,
                    last_reviewed_at_ms: None,
                    last_review_event_id: None,
                },
                card,
            }
        })
        .collect();
    PristineDeckImport {
        deck,
        notes: vec![PristineDeckNote { note, cards }],
    }
}

fn pristine_bundle_import() -> PristineBundleImport {
    let mut first = pristine_deck_import();
    first.deck.name = "Japanese 00".into();
    first.deck.language_tag = Some("ja-JP".into());

    let mut second = first.clone();
    second.deck.id = "deck-mixed-2".into();
    second.deck.name = "Japanese 01".into();
    for imported_note in &mut second.notes {
        let source = &mut imported_note.note.source_item;
        source.id.push_str("-2");
        source.deck_id.clone_from(&second.deck.id);
        for segment in &mut source.segments {
            segment.id.push_str("-2");
            if let SegmentContent::Cloze { cloze_id, .. } = &mut segment.content {
                cloze_id.push_str("-2");
            }
        }
        for annotation in &mut source.annotations {
            annotation.id.push_str("-2");
        }
        for media in &mut source.media {
            media.id.push_str("-2");
        }
        for tag in &mut source.tags {
            tag.id.push_str("-2");
            tag.name.push_str(" 2");
        }
        for cloze in &mut imported_note.note.clozes {
            cloze.id.push_str("-2");
            cloze.source_item_id.clone_from(&source.id);
            for annotation in &mut cloze.annotations {
                annotation.id.push_str("-2");
            }
            for media in &mut cloze.media {
                media.id.push_str("-2");
            }
        }
        for imported_card in &mut imported_note.cards {
            imported_card.card.id.push_str("-2");
            imported_card.card.cloze_id.push_str("-2");
            imported_card
                .initial_schedule
                .card_id
                .clone_from(&imported_card.card.id);
        }
    }

    PristineBundleImport {
        language_tag: "ja-JP".into(),
        decks: vec![first, second],
    }
}

fn leave_bundle_content_in_legacy_trash(
    storage: &mut Storage,
    bundle: &PristineBundleImport,
    deleted_at_ms: i64,
) {
    for stage in &bundle.decks {
        let card_ids = stage
            .notes
            .iter()
            .flat_map(|note| note.cards.iter().map(|card| card.card.id.clone()))
            .collect::<Vec<_>>();
        storage
            .move_deck_cards(&card_ids, DEFAULT_DECK_ID, deleted_at_ms)
            .unwrap();
        storage
            .set_deck_cards_deleted(&card_ids, Some(deleted_at_ms), deleted_at_ms)
            .unwrap();
        storage.delete_deck(&stage.deck.id).unwrap();
    }
    storage
        .connection
        .execute(
            "DELETE FROM bundle_installations WHERE language_tag = ?1",
            [&bundle.language_tag],
        )
        .unwrap();
}

#[test]
fn pristine_bundle_import_adds_only_missing_decks_with_associations_and_inherited_scheduling() {
    let mut storage = Storage::open_in_memory().unwrap();
    let mut bundle = pristine_bundle_import();
    bundle.decks[1].deck.created_at_ms = 1_500;
    bundle.decks[1].deck.updated_at_ms = 1_500;
    bundle.decks[1].notes[0].note.source_item.created_at_ms = 1_500;
    bundle.decks[1].notes[0].note.source_item.updated_at_ms = 1_500;
    let first_stage = PristineBundleImport {
        language_tag: bundle.language_tag.clone(),
        decks: vec![bundle.decks[0].clone()],
    };
    storage
        .import_pristine_bundle(&first_stage, || {}, || Ok::<(), ()>(()))
        .unwrap();

    let plan = storage.validate_pristine_bundle_import(&bundle).unwrap();
    assert_eq!(plan.installed_deck_ids, [bundle.decks[0].deck.id.clone()]);
    assert_eq!(plan.missing_deck_ids, [bundle.decks[1].deck.id.clone()]);
    assert!(plan.unassociated_deck_ids.is_empty());
    let mut imported_cards = 0;
    let (completed, ()) = storage
        .import_pristine_bundle(&bundle, || imported_cards += 1, || Ok::<(), ()>(()))
        .unwrap();
    assert_eq!(completed, plan);
    assert_eq!(imported_cards, 2);

    for stage in &bundle.decks {
        let profile = storage.get_scheduler_profile(&stage.deck.id).unwrap();
        assert_eq!(profile.scheduling_mode, SchedulingMode::Automatic);
        assert_eq!(profile.deck_daily_time_budget_minutes, None);
    }
    assert_eq!(
        storage
            .connection
            .query_row("SELECT COUNT(*) FROM bundle_installations", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap(),
        1
    );
    assert_eq!(
        storage
            .connection
            .query_row("SELECT COUNT(*) FROM bundle_decks", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap(),
        2
    );

    let mut unexpected_callback = false;
    let (no_op, ()) = storage
        .import_pristine_bundle(&bundle, || unexpected_callback = true, || Ok::<(), ()>(()))
        .unwrap();
    assert!(no_op.missing_deck_ids.is_empty());
    assert!(no_op.unassociated_deck_ids.is_empty());
    assert!(!unexpected_callback);
}

#[test]
fn pristine_bundle_validation_rejects_an_existing_deck_associated_with_another_stage() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    let mut reordered = bundle.clone();
    reordered.decks.swap(0, 1);

    assert!(matches!(
        storage.validate_pristine_bundle_import(&reordered),
        Err(StorageError::InvalidAggregate(message))
            if message.contains("associated with another bundle or stage")
    ));
}

#[test]
fn pristine_bundle_failure_rolls_back_all_decks_and_associations() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();

    assert!(matches!(
        storage.import_pristine_bundle_failing_before_commit(&bundle),
        Err(StorageError::InjectedTestFailure(
            "pristine bundle transaction before commit"
        ))
    ));
    for stage in &bundle.decks {
        assert!(matches!(
            storage.get_deck(&stage.deck.id),
            Err(StorageError::EntityNotFound { .. })
        ));
    }
    for table in ["bundle_installations", "bundle_decks"] {
        assert_eq!(
            storage
                .connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, u64>(0)
                })
                .unwrap(),
            0,
            "{table}"
        );
    }
}

#[test]
fn legacy_bundle_remnant_purge_failure_leaves_trashed_content_unchanged() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    leave_bundle_content_in_legacy_trash(&mut storage, &bundle, 3_000);
    let removed_notes = storage.library_notes().unwrap();

    assert!(matches!(
        storage.import_pristine_bundle_failing_before_commit(&bundle),
        Err(StorageError::InjectedTestFailure(
            "pristine bundle transaction before commit"
        ))
    ));
    assert_eq!(storage.library_notes().unwrap(), removed_notes);
    assert!(storage.installed_bundles().unwrap().is_empty());
    for stage in &bundle.decks {
        assert!(matches!(
            storage.get_deck(&stage.deck.id),
            Err(StorageError::EntityNotFound { .. })
        ));
    }
}

#[test]
fn exact_legacy_bundle_remnants_are_purged_before_a_fresh_import() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    let review = event_for_card(&storage, "pristine-card-0", "legacy-review", 3_000);
    storage.commit_review(&review).unwrap();
    storage
        .set_deck_cards_suspended(&["pristine-card-0".into()], true, 3_100)
        .unwrap();
    let mut changed = storage.get_card("pristine-card-0").unwrap();
    changed.content_version = 7;
    changed.updated_at_ms = 3_200;
    storage.update_card(&changed).unwrap();
    leave_bundle_content_in_legacy_trash(&mut storage, &bundle, 4_000);

    let plan = storage.validate_pristine_bundle_import(&bundle).unwrap();
    assert_eq!(plan.stale_source_ids.len(), 2);
    let mut imported_cards = 0;
    storage
        .import_pristine_bundle(&bundle, || imported_cards += 1, || Ok::<(), ()>(()))
        .unwrap();

    assert_eq!(imported_cards, 4);
    let fresh = storage.load_study_card("pristine-card-0").unwrap();
    assert!(!fresh.card.suspended);
    assert_eq!(fresh.card.content_version, 0);
    assert_eq!(fresh.schedule.lifecycle, CardLifecycle::Unseen);
    assert_eq!(fresh.schedule.version, 0);
    assert!(storage.review_events("pristine-card-0").unwrap().is_empty());
    for (table, id) in [
        ("source_items", "source-mixed"),
        ("cards", "pristine-card-0"),
        ("tags", "tag-mixed"),
        ("annotations", "annotation-source"),
        ("media_references", "media-source"),
    ] {
        assert_eq!(
            storage
                .connection
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE id = ?1"),
                    [id],
                    |row| row.get::<_, u64>(0),
                )
                .unwrap(),
            1,
            "{table}"
        );
    }
}

#[test]
fn pristine_bundle_import_rejects_an_active_identity_owned_by_another_bundle() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();

    let mut other_stage = bundle.decks[0].clone();
    other_stage.deck.id = "deck-other-bundle".into();
    other_stage.deck.name = "Korean 00".into();
    other_stage.deck.language_tag = Some("ko-KR".into());
    other_stage.notes.clear();
    storage
        .import_pristine_bundle(
            &PristineBundleImport {
                language_tag: "ko-KR".into(),
                decks: vec![other_stage.clone()],
            },
            || {},
            || Ok::<(), ()>(()),
        )
        .unwrap();
    storage
        .move_deck_cards(
            &["pristine-card-0".into(), "pristine-card-1".into()],
            &other_stage.deck.id,
            3_000,
        )
        .unwrap();
    storage
        .remove_bundle("ja-JP", 2, 2, 4_000, |_, _| {})
        .unwrap();

    assert!(matches!(
        storage.validate_pristine_bundle_import(&bundle),
        Err(StorageError::InvalidAggregate(message))
            if message.contains("active or differently owned content")
    ));
    for stage in &bundle.decks {
        assert!(matches!(
            storage.get_deck(&stage.deck.id),
            Err(StorageError::EntityNotFound { .. })
        ));
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn bundle_removal_uses_one_confirmation_for_six_stages_and_preserves_unrelated_content() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    for ordinal in 2..6 {
        let deck_id = format!("empty-bundle-stage-{ordinal}");
        let mut stage = deck(&deck_id);
        stage.name = format!("Japanese {ordinal:02}");
        stage.language_tag = Some(bundle.language_tag.clone());
        storage.create_deck(&stage).unwrap();
        storage
            .connection
            .execute(
                "INSERT INTO bundle_decks(language_tag, deck_id, ordinal)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![bundle.language_tag, deck_id, ordinal],
            )
            .unwrap();
    }

    let moved_out_review = event_for_card(
        &storage,
        "pristine-card-0",
        "moved-out-bundle-review",
        2_500,
    );
    storage.commit_review(&moved_out_review).unwrap();
    storage
        .move_deck_cards(
            &["pristine-card-0".into(), "pristine-card-1".into()],
            DEFAULT_DECK_ID,
            3_000,
        )
        .unwrap();
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], "deck-mixed-2", 3_000)
        .unwrap();
    let review = sample_event(&storage, "bundle-removal-review", 4_000);
    storage.commit_review(&review).unwrap();
    let preview = storage.installed_bundles().unwrap();
    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].language_tag, "ja-JP");
    assert_eq!(preview[0].deck_count, 6);
    assert_eq!(preview[0].active_card_count, 3);

    let mut progress = Vec::new();
    let removed = storage
        .remove_bundle("ja-JP", 6, 3, 5_000, |decks, cards| {
            progress.push((decks, cards));
        })
        .unwrap();
    assert_eq!(removed.language_tag, preview[0].language_tag);
    assert_eq!(removed.deck_count, preview[0].deck_count);
    assert_eq!(removed.active_card_count, preview[0].active_card_count);
    assert_eq!(progress.len(), 6);
    assert_eq!(progress.last(), Some(&(6, 3)));
    assert!(
        progress
            .windows(2)
            .all(|pair| pair[0].0 <= pair[1].0 && pair[0].1 <= pair[1].1)
    );
    assert!(storage.installed_bundles().unwrap().is_empty());
    for stage in &bundle.decks {
        assert!(matches!(
            storage.get_deck(&stage.deck.id),
            Err(StorageError::EntityNotFound { .. })
        ));
    }

    let notes = storage.library_notes().unwrap();
    let moved_out = notes
        .iter()
        .find(|note| note.note.source_item.id == "source-mixed")
        .unwrap();
    assert_eq!(moved_out.note.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(moved_out.deleted_at_ms, None);
    assert_eq!(
        storage.review_events("pristine-card-0").unwrap(),
        vec![moved_out_review]
    );
    let manually_added = notes
        .iter()
        .find(|note| note.note.source_item.id == SAMPLE_SOURCE_ID)
        .unwrap();
    assert_eq!(manually_added.note.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(manually_added.deleted_at_ms, Some(5_000));
    assert!(
        notes
            .iter()
            .all(|note| note.note.source_item.id != "source-mixed-2")
    );
    assert_eq!(storage.review_events(SAMPLE_CARD_ID).unwrap(), vec![review]);
    assert!(matches!(
        storage.get_media_reference("media-source-2"),
        Err(StorageError::EntityNotFound { .. })
    ));
    assert!(
        !removed
            .orphaned_media_hashes
            .contains(&"sha256-media-source".into())
    );
    assert_eq!(
        storage
            .media_reference_count_for_hash("sha256-media-source")
            .unwrap(),
        1
    );
}

#[test]
fn bundle_removal_removes_only_stages_that_remain_associated() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    storage
        .delete_deck_and_rehome_notes(&bundle.decks[0].deck.id, None, 3_000, |_, _| {})
        .unwrap();

    let preview = storage.installed_bundles().unwrap().remove(0);
    assert_eq!(preview.deck_count, 1);
    assert_eq!(preview.active_card_count, 2);
    storage
        .remove_bundle(
            &preview.language_tag,
            preview.deck_count,
            preview.active_card_count,
            4_000,
            |_, _| {},
        )
        .unwrap();

    assert!(storage.installed_bundles().unwrap().is_empty());
    assert!(storage.library_notes().unwrap().is_empty());
}

#[test]
fn deleting_an_imported_stage_purges_bundle_content_and_safely_trashes_personal_content() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    let bundle_review = event_for_card(
        &storage,
        "pristine-card-0",
        "stage-deletion-bundle-review",
        3_000,
    );
    storage.commit_review(&bundle_review).unwrap();
    let personal_review = sample_event(&storage, "stage-deletion-personal-review", 3_100);
    storage.commit_review(&personal_review).unwrap();
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], &bundle.decks[0].deck.id, 3_200)
        .unwrap();

    let first = storage
        .delete_deck_and_rehome_notes(&bundle.decks[0].deck.id, None, 4_000, |_, _| {})
        .unwrap();
    assert!(matches!(
        storage.get_source_note("source-mixed"),
        Err(StorageError::EntityNotFound { .. })
    ));
    assert!(storage.review_events("pristine-card-0").is_err());
    let personal = storage
        .library_notes()
        .unwrap()
        .into_iter()
        .find(|note| note.note.source_item.id == SAMPLE_SOURCE_ID)
        .unwrap();
    assert_eq!(personal.note.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(personal.deleted_at_ms, Some(4_000));
    assert_eq!(
        storage.review_events(SAMPLE_CARD_ID).unwrap(),
        vec![personal_review]
    );
    assert!(first.orphaned_media_hashes.is_empty());
    assert_eq!(storage.installed_bundles().unwrap()[0].deck_count, 1);

    let second = storage
        .delete_deck_and_rehome_notes(&bundle.decks[1].deck.id, None, 5_000, |_, _| {})
        .unwrap();
    assert!(storage.installed_bundles().unwrap().is_empty());
    assert!(matches!(
        storage.get_source_note("source-mixed-2"),
        Err(StorageError::EntityNotFound { .. })
    ));
    assert!(!second.orphaned_media_hashes.is_empty());
}

#[test]
fn batch_deletion_rejects_the_complete_invalid_set_before_writing() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    for (id, name) in [("first-deck", "First"), ("second-deck", "Second")] {
        storage
            .create_deck(&Deck {
                id: id.into(),
                name: name.into(),
                description: None,
                language_tag: None,
                direction: Direction::Auto,
                matching_policy: MatchingPolicy::Strict,
                settings: StudySettingsOverride::default(),
                created_at_ms: 1_000,
                updated_at_ms: 1_000,
            })
            .unwrap();
    }
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], "first-deck", 2_000)
        .unwrap();

    for invalid_ids in [
        vec!["first-deck".into(), "first-deck".into()],
        vec!["first-deck".into(), DEFAULT_DECK_ID.into()],
        vec!["first-deck".into(), "missing-deck".into()],
    ] {
        assert!(
            storage
                .delete_decks_and_rehome_notes(&invalid_ids, 3_000, |_, _| {})
                .is_err()
        );
        assert!(storage.get_deck("first-deck").is_ok());
        assert!(storage.get_deck("second-deck").is_ok());
        let note = storage.get_source_note(SAMPLE_SOURCE_ID).unwrap();
        assert_eq!(note.source_item.deck_id, "first-deck");
    }
}

#[test]
fn batch_deletion_preserves_mixed_semantics_and_shared_media() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    storage
        .create_deck(&Deck {
            id: "ordinary-deck".into(),
            name: "Ordinary".into(),
            description: None,
            language_tag: None,
            direction: Direction::Auto,
            matching_policy: MatchingPolicy::Strict,
            settings: StudySettingsOverride::default(),
            created_at_ms: 2_000,
            updated_at_ms: 2_000,
        })
        .unwrap();
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], &bundle.decks[1].deck.id, 2_500)
        .unwrap();
    let sample_schedule = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
    let unrelated_schedule = storage.load_schedule("pristine-card-0").unwrap();

    let mut progress = Vec::new();
    let deleted = storage
        .delete_decks_and_rehome_notes(
            &["ordinary-deck".into(), bundle.decks[1].deck.id.clone()],
            3_000,
            |current, total| progress.push((current, total)),
        )
        .unwrap();

    assert_eq!(deleted.deck_ids.len(), 2);
    assert_eq!(deleted.active_card_count, 3);
    assert_eq!(progress.first(), Some(&(0, 3)));
    assert_eq!(progress.last(), Some(&(3, 3)));
    assert!(storage.get_deck("ordinary-deck").is_err());
    assert!(storage.get_deck(&bundle.decks[1].deck.id).is_err());
    assert!(storage.get_deck(&bundle.decks[0].deck.id).is_ok());
    assert_eq!(storage.installed_bundles().unwrap()[0].deck_count, 1);
    assert!(storage.get_source_note("source-mixed-2").is_err());
    let personal = storage
        .library_notes()
        .unwrap()
        .into_iter()
        .find(|note| note.note.source_item.id == SAMPLE_SOURCE_ID)
        .unwrap();
    assert_eq!(personal.note.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(personal.deleted_at_ms, Some(3_000));
    assert_eq!(
        storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
        sample_schedule
    );
    let unrelated = storage.get_source_note("source-mixed").unwrap();
    assert_eq!(unrelated.source_item.deck_id, bundle.decks[0].deck.id);
    assert_eq!(
        storage.load_schedule("pristine-card-0").unwrap(),
        unrelated_schedule
    );
    assert_eq!(
        storage
            .media_reference_count_for_hash("sha256-media-source")
            .unwrap(),
        1
    );
    assert!(
        !deleted
            .orphaned_media_hashes
            .contains(&"sha256-media-source".into())
    );

    storage
        .delete_decks_and_rehome_notes(&[bundle.decks[0].deck.id.clone()], 4_000, |_, _| {})
        .unwrap();
    assert!(storage.installed_bundles().unwrap().is_empty());
}

#[test]
fn batch_deletion_moves_ordinary_content_to_trash_with_learning_state() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    for id in ["first-ordinary", "second-ordinary"] {
        let mut created = deck(id);
        created.name = id.into();
        storage.create_deck(&created).unwrap();
    }
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], "first-ordinary", 2_000)
        .unwrap();
    let review = sample_event(&storage, "ordinary-batch-review", 2_500);
    storage.commit_review(&review).unwrap();
    let schedule = storage.load_schedule(SAMPLE_CARD_ID).unwrap();

    let deleted = storage
        .delete_decks_and_rehome_notes(
            &["first-ordinary".into(), "second-ordinary".into()],
            3_000,
            |_, _| {},
        )
        .unwrap();

    assert_eq!(deleted.active_card_count, 1);
    let note = storage
        .library_notes()
        .unwrap()
        .into_iter()
        .find(|note| note.note.source_item.id == SAMPLE_SOURCE_ID)
        .unwrap();
    assert_eq!(note.note.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(note.deleted_at_ms, Some(3_000));
    assert_eq!(storage.load_schedule(SAMPLE_CARD_ID).unwrap(), schedule);
    assert_eq!(storage.review_events(SAMPLE_CARD_ID).unwrap(), vec![review]);
}

#[test]
fn batch_deletion_rolls_back_every_selected_deck() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    for id in ["first-deck", "second-deck"] {
        let mut created = deck(id);
        created.name = id.into();
        storage.create_deck(&created).unwrap();
    }
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], "first-deck", 2_000)
        .unwrap();
    storage
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_second_batch_deck_delete
             BEFORE DELETE ON decks
             WHEN OLD.id = 'second-deck'
             BEGIN
                 SELECT RAISE(ABORT, 'injected batch deletion failure');
             END;",
        )
        .unwrap();

    assert!(
        storage
            .delete_decks_and_rehome_notes(
                &["first-deck".into(), "second-deck".into()],
                3_000,
                |_, _| {},
            )
            .is_err()
    );
    assert!(storage.get_deck("first-deck").is_ok());
    assert!(storage.get_deck("second-deck").is_ok());
    let note = storage.get_source_note(SAMPLE_SOURCE_ID).unwrap();
    assert_eq!(note.source_item.deck_id, "first-deck");
}

#[test]
fn bundle_removal_rolls_back_every_stage_when_one_deck_fails() {
    let mut storage = Storage::open_in_memory().unwrap();
    let bundle = pristine_bundle_import();
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    let review = event_for_card(
        &storage,
        "pristine-card-0",
        "bundle-removal-rollback-review",
        2_500,
    );
    storage.commit_review(&review).unwrap();
    let notes_before = storage.library_notes().unwrap();
    storage
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_second_bundle_deck_delete
             BEFORE DELETE ON decks
             WHEN OLD.id = 'deck-mixed-2'
             BEGIN
                 SELECT RAISE(ABORT, 'injected bundle removal failure');
             END;",
        )
        .unwrap();

    assert!(
        storage
            .remove_bundle("ja-JP", 2, 4, 3_000, |_, _| {})
            .is_err()
    );
    assert_eq!(storage.installed_bundles().unwrap()[0].deck_count, 2);
    assert_eq!(storage.library_notes().unwrap(), notes_before);
    assert_eq!(storage.review_events("pristine-card-0").unwrap(), [review]);
    for stage in bundle.decks {
        assert_eq!(storage.get_deck(&stage.deck.id).unwrap(), stage.deck);
    }
}

#[test]
fn pristine_bundle_identity_collisions_leave_no_partial_decks() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();

    let mut source_collision = pristine_bundle_import();
    source_collision.decks[0].notes[0]
        .note
        .source_item
        .id
        .clone_from(&SAMPLE_SOURCE_ID.into());
    for cloze in &mut source_collision.decks[0].notes[0].note.clozes {
        cloze.source_item_id = SAMPLE_SOURCE_ID.into();
    }
    let mut cloze_collision = pristine_bundle_import();
    let original_cloze_id = cloze_collision.decks[0].notes[0].note.clozes[0].id.clone();
    cloze_collision.decks[0].notes[0].note.clozes[0].id = SAMPLE_CLOZE_ID.into();
    for segment in &mut cloze_collision.decks[0].notes[0].note.source_item.segments {
        if let SegmentContent::Cloze { cloze_id, .. } = &mut segment.content {
            if *cloze_id == original_cloze_id {
                *cloze_id = SAMPLE_CLOZE_ID.into();
            }
        }
    }
    cloze_collision.decks[0].notes[0].cards[0].card.cloze_id = SAMPLE_CLOZE_ID.into();
    let mut card_collision = pristine_bundle_import();
    card_collision.decks[0].notes[0].cards[0].card.id = SAMPLE_CARD_ID.into();
    card_collision.decks[0].notes[0].cards[0]
        .initial_schedule
        .card_id = SAMPLE_CARD_ID.into();
    for (collision, entity) in [
        (source_collision, "source note"),
        (cloze_collision, "cloze"),
        (card_collision, "card"),
    ] {
        let deck_id = collision.decks[0].deck.id.clone();
        assert!(matches!(
            storage.validate_pristine_bundle_import(&collision),
            Err(StorageError::InvalidAggregate(message))
                if message.contains(entity) && message.contains("already exists")
        ));
        assert!(matches!(
            storage.get_deck(&deck_id),
            Err(StorageError::EntityNotFound { .. })
        ));
    }

    let media_collision = pristine_bundle_import();
    let media = &media_collision.decks[0].notes[0].note.source_item.media[0];
    storage.create_media_reference(media).unwrap();
    assert!(matches!(
        storage.validate_pristine_bundle_import(&media_collision),
        Err(StorageError::InvalidAggregate(message))
            if message.contains("media reference") && message.contains("already exists")
    ));
    storage.delete_media_reference(&media.id).unwrap();

    assert_eq!(storage.library_notes().unwrap().len(), 1);
    assert_eq!(
        storage
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
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
fn version_twelve_tracks_only_existing_bundle_sources_from_each_stage_timestamp() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("bundle-v11.db");
    let mut storage = Storage::open(&path).unwrap();
    storage.seed_walking_skeleton(500).unwrap();
    let mut bundle = pristine_bundle_import();
    bundle.decks[1].deck.created_at_ms = 1_500;
    bundle.decks[1].deck.updated_at_ms = 1_500;
    bundle.decks[1].notes[0].note.source_item.created_at_ms = 1_500;
    bundle.decks[1].notes[0].note.source_item.updated_at_ms = 1_500;
    storage
        .import_pristine_bundle(&bundle, || {}, || Ok::<(), ()>(()))
        .unwrap();
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], &bundle.decks[0].deck.id, 2_000)
        .unwrap();
    storage
        .connection
        .execute_batch(
            "DROP TRIGGER review_events_are_append_only_delete;
             DROP TRIGGER bundle_source_notes_leave_stage;
             DROP TABLE bundle_source_notes;
             DROP INDEX semantic_segments_cloze;
             DROP INDEX source_item_tags_tag;
             DROP INDEX source_item_annotations_annotation;
             DROP INDEX cloze_annotations_annotation;
             DROP INDEX source_item_media_reference;
             DROP INDEX cloze_media_reference;
             CREATE TRIGGER review_events_are_append_only_delete
             BEFORE DELETE ON review_events
             BEGIN
                 SELECT RAISE(ABORT, 'review events are append-only');
             END;
             DELETE FROM schema_migrations WHERE version >= 12;",
        )
        .unwrap();
    drop(storage);

    let migrated = Storage::open(&path).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), 13);
    let tracked = {
        let mut statement = migrated
            .connection
            .prepare("SELECT source_item_id FROM bundle_source_notes ORDER BY source_item_id")
            .unwrap();
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    assert_eq!(tracked, ["source-mixed", "source-mixed-2"]);
    assert!(!tracked.contains(&SAMPLE_SOURCE_ID.into()));
}

#[test]
fn version_thirteen_indexes_reverse_references_used_by_deletion() {
    let storage = Storage::open_in_memory().unwrap();
    let mut statement = storage
        .connection
        .prepare(
            "SELECT name
             FROM sqlite_schema
             WHERE type = 'index'
               AND name IN (
                   'semantic_segments_cloze',
                   'source_item_tags_tag',
                   'source_item_annotations_annotation',
                   'cloze_annotations_annotation',
                   'source_item_media_reference',
                   'cloze_media_reference'
               )
             ORDER BY name",
        )
        .unwrap();
    let indexes = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(indexes.len(), 6);
}

#[test]
fn wal_writer_process() {
    let Some(path) = std::env::var_os("MEIKI_TEST_WAL_WRITER_PATH") else {
        return;
    };
    let path = std::path::PathBuf::from(path);
    let mut storage = Storage::open(&path).unwrap();
    storage
        .connection
        .execute_batch("PRAGMA wal_autocheckpoint = 0;")
        .unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let event = sample_event(&storage, "wal-review", 10_000);
    storage.commit_review(&event).unwrap();
    std::process::exit(0);
}

#[test]
fn committed_wal_recovers_after_the_writer_process_terminates() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("terminated-writer.db");
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("tests::wal_writer_process")
        .arg("--nocapture")
        .env("MEIKI_TEST_WAL_WRITER_PATH", &path)
        .status()
        .unwrap();
    assert!(status.success());
    let wal_path = path.with_file_name(format!(
        "{}-wal",
        path.file_name().unwrap().to_string_lossy()
    ));
    assert!(
        wal_path.is_file(),
        "the terminated writer left a WAL to recover"
    );

    let recovered = Storage::open(&path).unwrap();
    assert_eq!(
        recovered
            .review_events(SAMPLE_CARD_ID)
            .unwrap()
            .iter()
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>(),
        ["wal-review"]
    );
    assert!(
        recovered
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
}

#[test]
fn released_v0_1_schema_fixture_opens_and_migrates() {
    const RELEASED_V0_1_SCHEMA: &[u8] = include_bytes!("../fixtures/released/v0.1-schema-7.db");
    let directory = tempdir().unwrap();
    let path = directory.path().join("released-v0.1.db");
    std::fs::write(&path, RELEASED_V0_1_SCHEMA).unwrap();

    let storage = Storage::open(&path).unwrap();
    assert_eq!(storage.schema_version().unwrap(), 13);
    assert_eq!(storage.get_deck(DEFAULT_DECK_ID).unwrap().name, "Default");
    assert_eq!(
        storage
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        storage
            .connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .next()
            .is_none()
    );
    for (table, expected) in [
        ("decks", 1_u64),
        ("source_items", 0),
        ("clozes", 0),
        ("cards", 0),
        ("review_events", 0),
        ("media_references", 0),
    ] {
        assert_eq!(
            storage
                .connection
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, u64>(0)
                })
                .unwrap(),
            expected,
            "{table}"
        );
    }
    assert!(
        storage
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
    let migration_count = storage
        .connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get::<_, u64>(0)
        })
        .unwrap();
    let backup = directory.path().join("released-v0.1.backup.db");
    storage.backup_to(&backup).unwrap();
    drop(storage);

    let reopened = Storage::open(&path).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 13);
    assert_eq!(
        reopened
            .connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get::<_, u64>(0)
            })
            .unwrap(),
        migration_count
    );
    drop(reopened);

    let restored_path = directory.path().join("released-v0.1-restored.db");
    let restored = Storage::restore_from_backup(&backup, &restored_path).unwrap();
    assert_eq!(restored.schema_version().unwrap(), 13);
    assert!(!restored.has_learning_material().unwrap());
    assert!(
        restored
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
    assert_eq!(
        migration_backup_schema_version(directory.path(), "released-v0.1.db.migration-v7-"),
        7
    );
}

#[test]
fn newer_schema_fails_without_migration_or_backup_writes() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("future.db");
    drop(Storage::open(&path).unwrap());
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO schema_migrations(version, applied_at_ms) VALUES (999, 42)",
            [],
        )
        .unwrap();
    let migrations_before = connection
        .query_row(
            "SELECT COUNT(*), MAX(version) FROM schema_migrations",
            [],
            |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
        )
        .unwrap();
    drop(connection);

    assert!(matches!(
        Storage::open(&path),
        Err(StorageError::UnsupportedSchema {
            found: 999,
            supported: 13
        })
    ));
    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*), MAX(version) FROM schema_migrations",
                [],
                |row| Ok((row.get::<_, u64>(0)?, row.get::<_, u32>(1)?)),
            )
            .unwrap(),
        migrations_before
    );
    assert!(!directory.path().join("backups").exists());
}

#[test]
fn unique_check_and_foreign_key_failures_leave_the_collection_valid() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    assert!(
        storage
            .connection
            .execute(
                "INSERT INTO semantic_segments(
                    id, source_item_id, ordinal, kind, text, cloze_id
                 ) VALUES ('duplicate-ordinal', ?1, 0, 'text', 'duplicate', NULL)",
                [SAMPLE_SOURCE_ID],
            )
            .is_err()
    );
    assert!(
        storage
            .connection
            .execute(
                "UPDATE cards SET suspended = 2 WHERE id = ?1",
                [SAMPLE_CARD_ID],
            )
            .is_err()
    );
    assert!(
        storage
            .connection
            .execute(
                "INSERT INTO cards(
                    id, cloze_id, content_version, created_at_ms, updated_at_ms,
                    suspended, queue_updated_at_ms
                 ) VALUES ('orphan-card', 'missing-cloze', 0, 1, 1, 0, 1)",
                [],
            )
            .is_err()
    );
    assert_eq!(
        storage
            .connection
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(
        storage
            .connection
            .prepare("PRAGMA foreign_key_check")
            .unwrap()
            .query_map([], |_| Ok(()))
            .unwrap()
            .next()
            .is_none()
    );
    assert!(!storage.get_card(SAMPLE_CARD_ID).unwrap().suspended);
}

#[test]
fn two_connections_reject_a_stale_concurrent_review_without_partial_state() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("concurrent.db");
    let mut first = Storage::open(&path).unwrap();
    first.seed_walking_skeleton(1_000).unwrap();
    let mut second = Storage::open(&path).unwrap();
    let winner = sample_event(&first, "concurrent-winner", 10_000);
    let mut stale = sample_event(&second, "concurrent-stale", 10_001);
    stale.previous_schedule = winner.previous_schedule.clone();
    stale.next_schedule.version = winner.next_schedule.version;

    first.commit_review(&winner).unwrap();
    assert!(matches!(
        second.commit_review(&stale),
        Err(StorageError::StaleReview)
    ));
    assert_eq!(
        first
            .review_events(SAMPLE_CARD_ID)
            .unwrap()
            .iter()
            .map(|event| event.id.as_str())
            .collect::<Vec<_>>(),
        ["concurrent-winner"]
    );
    assert_eq!(
        first.load_schedule(SAMPLE_CARD_ID).unwrap(),
        winner.next_schedule
    );
}

#[test]
fn version_ten_migrates_legacy_policy_without_losing_user_choices() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("policy-v9.db");
    let mut legacy = open_version_nine_fixture(&path);
    legacy
        .connection
        .execute(
            "UPDATE scheduler_profiles
             SET intensity = 'intensive', daily_time_budget_minutes = 45
             WHERE deck_id = ?1",
            [DEFAULT_DECK_ID],
        )
        .unwrap();
    legacy.create_deck(&deck("manual-deck")).unwrap();
    legacy
        .connection
        .execute(
            "UPDATE scheduler_profiles
             SET daily_time_budget_minutes = 25
             WHERE deck_id = 'manual-deck'",
            [],
        )
        .unwrap();
    drop(legacy);

    let migrated = Storage::open(&path).unwrap();
    assert_eq!(migrated.schema_version().unwrap(), 13);
    assert_eq!(
        migrated
            .collection_scheduling_settings()
            .unwrap()
            .daily_time_budget_minutes,
        45
    );
    let automatic = migrated.get_scheduler_profile(DEFAULT_DECK_ID).unwrap();
    assert_eq!(
        automatic.scheduling_mode,
        meiki_domain::SchedulingMode::Automatic
    );
    assert_eq!(automatic.deck_daily_time_budget_minutes, None);
    assert_eq!(automatic.controller_target_retention_basis_points, 9_300);

    let expert = migrated.get_scheduler_profile("manual-deck").unwrap();
    assert_eq!(expert.scheduling_mode, meiki_domain::SchedulingMode::Expert);
    assert_eq!(expert.deck_daily_time_budget_minutes, Some(25));
    assert_eq!(expert.controller_target_retention_basis_points, 9_200);
    assert_eq!(expert.controller_new_cards_per_day, 12);
    assert_eq!(
        migration_backup_schema_version(directory.path(), "policy-v9.db.migration-v9-"),
        9
    );
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
    event.chosen_grade = Grade::Easy;
    event.grade_overridden = true;
    event.response_duration_ms = 1_234;

    let committed = storage.commit_review(&event).unwrap();
    assert_eq!(committed.version, 1);
    assert_eq!(committed.lifecycle, CardLifecycle::Introduced);
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
#[allow(clippy::too_many_lines)]
fn fixed_lifecycle_command_model_preserves_durable_invariants_after_every_step() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("lifecycle-model.db");
    let mut storage = Storage::open(&path).unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let mut model = LifecycleModel {
        active_reviews: 0,
        event_count: 0,
        suspended: false,
        trashed: false,
        content_version: 0,
    };
    assert_lifecycle_model(&storage, &model);

    let first = sample_event(&storage, "model-review-1", 10_000);
    storage.commit_review(&first).unwrap();
    model.active_reviews = 1;
    model.event_count = 1;
    assert_lifecycle_model(&storage, &model);

    assert!(matches!(
        storage.commit_review(&first),
        Err(StorageError::StaleReview)
    ));
    assert_lifecycle_model(&storage, &model);
    assert!(matches!(
        storage.undo_last_review(
            SAMPLE_CARD_ID,
            "not-the-latest-review",
            "model-invalid-undo",
            11_000
        ),
        Err(StorageError::StaleReview)
    ));
    assert_lifecycle_model(&storage, &model);

    storage
        .undo_last_review(SAMPLE_CARD_ID, "model-review-1", "model-undo-1", 12_000)
        .unwrap();
    model.active_reviews = 0;
    model.event_count = 2;
    assert_lifecycle_model(&storage, &model);

    storage
        .set_deck_cards_suspended(&[SAMPLE_CARD_ID.into()], true, 13_000)
        .unwrap();
    model.suspended = true;
    assert_lifecycle_model(&storage, &model);
    storage
        .set_deck_cards_suspended(&[SAMPLE_CARD_ID.into()], false, 14_000)
        .unwrap();
    model.suspended = false;
    assert_lifecycle_model(&storage, &model);

    let mut card = storage.get_card(SAMPLE_CARD_ID).unwrap();
    card.content_version += 1;
    card.updated_at_ms = 15_000;
    storage.update_card(&card).unwrap();
    model.content_version = 1;
    assert_lifecycle_model(&storage, &model);

    storage
        .set_deck_cards_deleted(&[SAMPLE_CARD_ID.into()], Some(16_000), 16_000)
        .unwrap();
    model.trashed = true;
    assert_lifecycle_model(&storage, &model);
    storage
        .set_deck_cards_deleted(&[SAMPLE_CARD_ID.into()], None, 17_000)
        .unwrap();
    model.trashed = false;
    assert_lifecycle_model(&storage, &model);

    let second = sample_event(&storage, "model-review-2", 20_000);
    storage.commit_review(&second).unwrap();
    model.active_reviews = 1;
    model.event_count = 3;
    assert_lifecycle_model(&storage, &model);
    drop(storage);

    let mut storage = Storage::open(&path).unwrap();
    assert!(matches!(
        storage.commit_review(&second),
        Err(StorageError::StaleReview)
    ));
    assert_lifecycle_model(&storage, &model);

    storage
        .connection
        .execute(
            "UPDATE schedule_states SET due_at_ms = -1 WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
        )
        .unwrap();
    assert_eq!(
        storage
            .check_collection_schedule_integrity()
            .unwrap()
            .mismatched_card_ids,
        [SAMPLE_CARD_ID]
    );
    storage.rebuild_schedule_projection(SAMPLE_CARD_ID).unwrap();
    assert_lifecycle_model(&storage, &model);

    assert!(
        storage
            .connection
            .execute(
                "UPDATE review_events
                 SET raw_response = 'mutated'
                 WHERE id = 'model-review-2'",
                [],
            )
            .is_err()
    );
    assert!(
        storage
            .connection
            .execute("DELETE FROM review_events WHERE id = 'model-review-2'", [],)
            .is_err()
    );
    assert_lifecycle_model(&storage, &model);
}

#[test]
fn injected_failure_before_review_commit_rolls_back_every_write() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let before = storage.load_study_card(SAMPLE_CARD_ID).unwrap();
    let event = sample_event(&storage, "review-before-commit-failure", 10_000);

    assert!(matches!(
        storage.commit_review_failing_before_commit(&event),
        Err(StorageError::InjectedTestFailure(
            "review transaction before commit"
        ))
    ));
    assert_eq!(storage.load_study_card(SAMPLE_CARD_ID).unwrap(), before);
    assert!(storage.review_events(SAMPLE_CARD_ID).unwrap().is_empty());
}

#[test]
fn failed_deck_deletion_rolls_back_trash_and_rehome_writes() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    storage
        .create_deck(&Deck {
            id: "doomed-deck".into(),
            name: "Doomed".into(),
            description: None,
            language_tag: None,
            direction: Direction::Auto,
            matching_policy: MatchingPolicy::Strict,
            settings: StudySettingsOverride::default(),
            created_at_ms: 1_000,
            updated_at_ms: 1_000,
        })
        .unwrap();
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], "doomed-deck", 2_000)
        .unwrap();
    storage
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_doomed_deck_delete
             BEFORE DELETE ON decks
             WHEN OLD.id = 'doomed-deck'
             BEGIN
                 SELECT RAISE(ABORT, 'injected deck deletion failure');
             END;",
        )
        .unwrap();

    let mut progress = Vec::new();
    assert!(
        storage
            .delete_deck_and_rehome_notes("doomed-deck", None, 3_000, |current, total| {
                progress.push((current, total));
            })
            .is_err()
    );
    assert_eq!(progress, [(0, 1)]);
    assert_eq!(storage.get_deck("doomed-deck").unwrap().name, "Doomed");
    let note = storage
        .library_notes()
        .unwrap()
        .into_iter()
        .find(|note| note.note.source_item.id == SAMPLE_SOURCE_ID)
        .unwrap();
    assert_eq!(note.note.source_item.deck_id, "doomed-deck");
    assert_eq!(note.deleted_at_ms, None);
}

#[test]
fn scheduling_workload_uses_aggregates_and_a_bounded_response_median() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let empty_history = storage
        .scheduling_workload(DEFAULT_DECK_ID, 10_000, 2_419_210_000)
        .unwrap();
    assert_eq!(empty_history.unseen_cards, 1);
    assert_eq!(empty_history.due_cards_now, 0);
    assert_eq!(empty_history.forecast_review_occurrences, 0);
    assert_eq!(empty_history.median_response_duration_ms, None);

    let mut event = sample_event(&storage, "workload-review", 10_000);
    event.response_duration_ms = 1_500;
    storage.commit_review(&event).unwrap();
    let workload = storage
        .scheduling_workload(DEFAULT_DECK_ID, 10_000, 2_419_210_000)
        .unwrap();
    assert_eq!(workload.unseen_cards, 0);
    assert_eq!(workload.due_cards_now, 0);
    assert!(workload.forecast_review_occurrences > 0);
    assert_eq!(workload.response_duration_samples, 1);
    assert_eq!(workload.median_response_duration_ms, Some(1_500));
    assert_eq!(workload.review_count, 1);
}

#[test]
fn undo_appends_a_compensating_event_and_restores_the_projection() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let initial = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
    let review = sample_event(&storage, "review-1", 10_000);
    let reviewed = storage.commit_review(&review).unwrap();
    assert_eq!(initial.lifecycle, CardLifecycle::Unseen);
    assert_eq!(reviewed.lifecycle, CardLifecycle::Introduced);

    let undone = storage
        .undo_last_review(SAMPLE_CARD_ID, "review-1", "undo-1", 11_000)
        .unwrap();
    assert_eq!(undone.version, reviewed.version + 1);
    assert_eq!(undone.due_at_ms, initial.due_at_ms);
    assert_eq!(undone.interval_milliseconds, initial.interval_milliseconds);
    assert_eq!(undone.repetitions, initial.repetitions);
    assert_eq!(undone.lifecycle, CardLifecycle::Unseen);
    assert_eq!(undone.last_reviewed_at_ms, initial.last_reviewed_at_ms);
    assert_eq!(undone.last_review_event_id.as_deref(), Some("undo-1"));
    assert_eq!(storage.load_schedule(SAMPLE_CARD_ID).unwrap(), undone);
    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 0);
    assert!(
        storage
            .active_review_events(SAMPLE_CARD_ID)
            .unwrap()
            .is_empty()
    );

    let history = storage.review_events(SAMPLE_CARD_ID).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].kind, ReviewEventKind::Undo);
    assert_eq!(
        history[1].undoes_review_event_id.as_deref(),
        Some("review-1")
    );

    storage
        .connection
        .execute(
            "UPDATE schedule_states
             SET version = 99, due_at_ms = -1, last_review_event_id = NULL
             WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
        )
        .unwrap();
    assert_eq!(
        storage.rebuild_schedule_projection(SAMPLE_CARD_ID).unwrap(),
        undone
    );
    assert!(matches!(
        storage.undo_last_review(SAMPLE_CARD_ID, "review-1", "undo-2", 12_000),
        Err(StorageError::NothingToUndo(card_id)) if card_id == SAMPLE_CARD_ID
    ));
}

#[test]
fn undoing_latest_of_multiple_reviews_keeps_the_card_introduced() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let first = sample_event(&storage, "review-1", 10_000);
    let after_first = storage.commit_review(&first).unwrap();
    let second = sample_event(&storage, "review-2", 20_000);
    let after_second = storage.commit_review(&second).unwrap();

    let restored = storage
        .undo_last_review(SAMPLE_CARD_ID, "review-2", "undo-2", 21_000)
        .unwrap();

    assert_eq!(after_first.lifecycle, CardLifecycle::Introduced);
    assert_eq!(after_second.lifecycle, CardLifecycle::Introduced);
    assert_eq!(restored.lifecycle, CardLifecycle::Introduced);
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

    let report = storage.check_collection_schedule_integrity().unwrap();
    assert_eq!(report.checked_cards, 1);
    assert_eq!(report.mismatched_card_ids, vec![SAMPLE_CARD_ID]);
    assert_eq!(
        storage
            .check_deck_schedule_integrity(DEFAULT_DECK_ID)
            .unwrap(),
        report
    );
    let rebuilt = storage.rebuild_schedule_projection(SAMPLE_CARD_ID).unwrap();
    assert_eq!(rebuilt, expected);
    assert_eq!(rebuilt.lifecycle, CardLifecycle::Introduced);
    assert_eq!(storage.load_schedule(SAMPLE_CARD_ID).unwrap(), expected);
    assert!(
        storage
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
    assert_eq!(
        storage.rebuild_schedule_projection(SAMPLE_CARD_ID).unwrap(),
        expected
    );

    let continued = sample_event(&storage, "review-after-repair", 20_000);
    let reviewed = storage.commit_review(&continued).unwrap();
    let undone = storage
        .undo_last_review(
            SAMPLE_CARD_ID,
            "review-after-repair",
            "undo-after-repair",
            21_000,
        )
        .unwrap();
    assert_eq!(reviewed.version + 1, undone.version);
    assert!(
        storage
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
}

#[test]
fn version_nine_migration_backs_up_and_repairs_only_from_recorded_snapshots() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("collection.db");
    let mut storage = open_version_eight_fixture(&path);
    storage.seed_walking_skeleton(1_000).unwrap();
    let event = sample_event(&storage, "review-before-migration", 10_000);
    let expected = storage.commit_review(&event).unwrap();
    let events_before = storage.review_events(SAMPLE_CARD_ID).unwrap();
    storage
        .connection
        .execute(
            "UPDATE schedule_states
             SET due_at_ms = -1,
                 ideal_due_at_ms = -2,
                 stability_milliseconds = 1,
                 difficulty_millipoints = 1000
             WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
        )
        .unwrap();
    drop(storage);

    let repaired = Storage::open(&path).unwrap();
    assert_eq!(repaired.schema_version().unwrap(), 13);
    assert_eq!(repaired.projection_migration_repaired_cards().unwrap(), 1);
    assert_eq!(repaired.load_schedule(SAMPLE_CARD_ID).unwrap(), expected);
    assert_eq!(
        repaired.review_events(SAMPLE_CARD_ID).unwrap(),
        events_before
    );
    assert!(
        repaired
            .check_collection_schedule_integrity()
            .unwrap()
            .is_valid()
    );
    assert_eq!(
        migration_backup_schema_version(directory.path(), "collection.db.migration-v8-"),
        8
    );
}

#[test]
fn failed_projection_migration_keeps_the_version_eight_collection_unchanged() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("collection.db");
    let mut storage = open_version_eight_fixture(&path);
    storage.seed_walking_skeleton(1_000).unwrap();
    let event = sample_event(&storage, "review-before-failure", 10_000);
    storage.commit_review(&event).unwrap();
    storage
        .connection
        .execute(
            "UPDATE schedule_states SET due_at_ms = -1 WHERE card_id = ?1",
            [SAMPLE_CARD_ID],
        )
        .unwrap();
    storage
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_projection_repair
             BEFORE UPDATE ON schedule_states
             BEGIN
                 SELECT RAISE(ABORT, 'injected projection repair failure');
             END;",
        )
        .unwrap();
    drop(storage);

    let Err(error) = Storage::open(&path) else {
        panic!("projection repair unexpectedly succeeded");
    };
    assert!(
        error
            .to_string()
            .contains("injected projection repair failure")
    );

    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get::<_, u32>(0),
            )
            .unwrap(),
        8
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT due_at_ms FROM schedule_states WHERE card_id = ?1",
                [SAMPLE_CARD_ID],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        -1
    );
    assert_eq!(
        migration_backup_schema_version(directory.path(), "collection.db.migration-v8-"),
        8
    );
}

#[test]
fn commit_and_undo_reject_a_malformed_existing_event_chain() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let first = sample_event(&storage, "review-1", 10_000);
    storage.commit_review(&first).unwrap();
    let second = sample_event(&storage, "review-2", 20_000);
    storage.commit_review(&second).unwrap();
    storage
        .connection
        .execute_batch("DROP TRIGGER review_events_are_append_only_update;")
        .unwrap();
    storage
        .connection
        .execute(
            "UPDATE review_events
             SET previous_schedule_version = 7
             WHERE id = 'review-2'",
            [],
        )
        .unwrap();
    let before = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
    let next = sample_event(&storage, "review-3", 30_000);

    assert!(matches!(
        storage.check_collection_schedule_integrity(),
        Err(StorageError::ProjectionMismatch(_))
    ));
    assert!(matches!(
        storage.commit_review(&next),
        Err(StorageError::ProjectionMismatch(_))
    ));
    assert!(matches!(
        storage.undo_last_review(SAMPLE_CARD_ID, "review-2", "undo-2", 31_000),
        Err(StorageError::ProjectionMismatch(_))
    ));
    assert_eq!(storage.load_schedule(SAMPLE_CARD_ID).unwrap(), before);
    assert_eq!(storage.review_events(SAMPLE_CARD_ID).unwrap().len(), 2);
}

#[test]
#[allow(clippy::too_many_lines)]
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
                 ) VALUES ('legacy-card', 2, 3000, 1, 0, 'legacy-lapse');
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
                 );
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
                    'legacy-lapse',
                    'legacy-card',
                    0,
                    '',
                    '',
                    'incorrect',
                    'again',
                    'again',
                    2500,
                    'legacy-scheduler',
                    1,
                    2000,
                    1,
                    1,
                    2,
                    3000,
                    1,
                    0
                 );",
            )
            .unwrap();
    }

    let mut storage = Storage::open(&path).unwrap();
    assert_eq!(storage.schema_version().unwrap(), 13);
    assert_eq!(
        migration_backup_schema_version(directory.path(), "v1.db.migration-v1-"),
        1
    );
    let restored = storage.load_study_card("legacy-card").unwrap();
    assert!(!restored.card.suspended);
    assert_eq!(restored.source_item.deck_id, DEFAULT_DECK_ID);
    assert_eq!(restored.source_item.direction, Direction::RightToLeft);
    assert_eq!(restored.cloze.answer, "کتاب");
    let legacy_events = storage.review_events("legacy-card").unwrap();
    assert_eq!(legacy_events.len(), 2);
    assert_eq!(legacy_events[0].kind, ReviewEventKind::Review);
    assert_eq!(legacy_events[0].response_duration_ms, 0);
    assert!(!legacy_events[0].grade_overridden);
    assert_eq!(
        legacy_events[0].previous_schedule.lifecycle,
        CardLifecycle::Unseen
    );
    assert_eq!(
        legacy_events[0].next_schedule.lifecycle,
        CardLifecycle::Introduced
    );
    assert_eq!(
        legacy_events[1].previous_schedule.lifecycle,
        CardLifecycle::Introduced
    );
    assert_eq!(
        legacy_events[1].next_schedule.lifecycle,
        CardLifecycle::Introduced
    );
    let baseline = storage
        .rebuildable_baseline_for_test("legacy-card")
        .unwrap();
    assert_eq!(baseline.version, 0);
    assert_eq!(baseline.due_at_ms, 1_000);
    assert_eq!(baseline.lifecycle, CardLifecycle::Unseen);
    let before = storage.load_schedule("legacy-card").unwrap();
    assert_eq!(before.repetitions, 0);
    assert_eq!(before.lifecycle, CardLifecycle::Introduced);
    assert_eq!(
        storage.rebuild_schedule_projection("legacy-card").unwrap(),
        before
    );
}

#[test]
fn version_five_media_migrates_to_roles_and_technical_metadata() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("legacy-media.db");
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch(FOUNDATION_MIGRATION).unwrap();
    connection.execute_batch(CORE_MODEL_MIGRATION).unwrap();
    connection
        .execute_batch(AUTHORING_DEFAULTS_MIGRATION)
        .unwrap();
    connection.execute_batch(FSRS7_SCHEDULER_MIGRATION).unwrap();
    connection.execute_batch(STUDY_SESSION_MIGRATION).unwrap();
    connection
        .execute(
            "INSERT INTO media_references(
                id, content_hash, kind, media_type, direction, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, 'auto', 1000)",
            rusqlite::params!["legacy-image", "sha256:legacy-image", "image", "image/png"],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO media_references(
                id, content_hash, kind, media_type, direction, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, 'auto', 1000)",
            rusqlite::params!["legacy-audio", "sha256:legacy-audio", "audio", "audio/ogg"],
        )
        .unwrap();
    drop(connection);

    let storage = Storage::open(&path).unwrap();
    assert_eq!(storage.schema_version().unwrap(), 13);
    assert_eq!(
        migration_backup_schema_version(directory.path(), "legacy-media.db.migration-v5-"),
        5
    );
    let image = storage.get_media_reference("legacy-image").unwrap();
    assert_eq!(image.role, MediaRole::RevealImage);
    assert_eq!(image.byte_size, 0);
    assert_eq!(image.width, None);
    let audio = storage.get_media_reference("legacy-audio").unwrap();
    assert_eq!(audio.role, MediaRole::AnswerAudio);
    assert_eq!(audio.duration_ms, None);
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
fn rolling_backups_prune_oldest_and_replace_an_existing_collection() {
    let directory = tempdir().unwrap();
    let source_path = directory.path().join("source.db");
    let destination_path = directory.path().join("destination.db");
    let backup;
    {
        let mut source = Storage::open(&source_path).unwrap();
        source.seed_walking_skeleton(1_000).unwrap();
        for _ in 0..5 {
            source
                .create_rolling_backup(&source_path, "test", 3)
                .unwrap();
        }
        let backups = std::fs::read_dir(directory.path().join("backups"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(backups.len(), 3);
        backup = backups
            .into_iter()
            .max_by_key(std::fs::DirEntry::file_name)
            .unwrap()
            .path();
    }
    drop(Storage::open(&destination_path).unwrap());

    let replaced = Storage::replace_from_backup(&backup, &destination_path).unwrap();
    assert_eq!(
        replaced
            .load_study_card(SAMPLE_CARD_ID)
            .unwrap()
            .cloze
            .answer,
        "行きます"
    );
}

#[test]
fn portable_history_restore_preserves_events_and_projection() {
    let mut source = Storage::open_in_memory().unwrap();
    source.seed_walking_skeleton(1_000).unwrap();
    let event = sample_event(&source, "portable-review", 10_000);
    let expected = source.commit_review(&event).unwrap();
    let baseline = source.load_schedule_baseline(SAMPLE_CARD_ID).unwrap();
    let events = source.review_events(SAMPLE_CARD_ID).unwrap();

    let mut target = Storage::open_in_memory().unwrap();
    target.seed_walking_skeleton(1_000).unwrap();
    target
        .restore_card_history(SAMPLE_CARD_ID, &baseline, &expected, &events)
        .unwrap();

    assert_eq!(target.load_schedule(SAMPLE_CARD_ID).unwrap(), expected);
    assert_eq!(expected.lifecycle, CardLifecycle::Introduced);
    assert_eq!(target.review_events(SAMPLE_CARD_ID).unwrap(), events);
}

#[test]
fn mutable_core_entities_support_create_read_update_delete() {
    let mut storage = Storage::open_in_memory().unwrap();

    let mut deck = deck("deck-crud");
    storage.create_deck(&deck).unwrap();
    assert_eq!(storage.get_deck(&deck.id).unwrap(), deck);
    assert!(storage.list_decks().unwrap().contains(&deck));
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
fn media_reference_deletion_requires_unlinking_and_hash_counts_track_deduplication() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.create_deck(&deck("deck-mixed")).unwrap();
    let mut note = mixed_note();
    storage.create_source_note(&note).unwrap();

    assert_eq!(storage.media_reference_usage("media-cloze").unwrap(), 1);
    assert!(matches!(
        storage.delete_media_reference("media-cloze"),
        Err(StorageError::MediaInUse { references: 1, .. })
    ));

    let mut duplicate = media("media-duplicate", MediaKind::Audio);
    duplicate.content_hash = note.clozes[0].media[0].content_hash.clone();
    storage.create_media_reference(&duplicate).unwrap();
    assert_eq!(
        storage
            .media_reference_count_for_hash(&duplicate.content_hash)
            .unwrap(),
        2
    );
    storage.delete_media_reference(&duplicate.id).unwrap();
    assert_eq!(
        storage
            .media_reference_count_for_hash(&duplicate.content_hash)
            .unwrap(),
        1
    );

    note.clozes[0].media.clear();
    storage.update_source_note(&note).unwrap();
    assert_eq!(storage.media_reference_usage("media-cloze").unwrap(), 0);
    storage.delete_media_reference("media-cloze").unwrap();
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
            suspended: false,
            created_at_ms: 1_000,
            updated_at_ms: 1_000,
        };
        let schedule = ScheduleState {
            card_id: card.id.clone(),
            version: 0,
            lifecycle: CardLifecycle::Unseen,
            due_at_ms: 1_000,
            ideal_due_at_ms: 1_000,
            interval_milliseconds: 0,
            interval_seconds: 0,
            repetitions: 0,
            stability_milliseconds: 0,
            difficulty_millipoints: 0,
            last_reviewed_at_ms: None,
            last_review_event_id: None,
        };
        storage.create_card(&card, &schedule).unwrap();
        assert_eq!(storage.get_card(&card.id).unwrap(), card);
        card.content_version = 1;
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

#[test]
#[allow(clippy::too_many_lines)]
fn deck_card_actions_are_atomic_and_preserve_history_and_media() {
    let mut storage = Storage::open_in_memory().unwrap();
    storage.seed_walking_skeleton(1_000).unwrap();
    let source_id = storage.library_notes().unwrap()[0]
        .note
        .source_item
        .id
        .clone();
    let mut note = storage.get_source_note(&source_id).unwrap();
    note.source_item
        .media
        .push(media("media-library", MediaKind::Image));
    storage.update_source_note(&note).unwrap();

    let review = sample_event(&storage, "review-library", 10_000);
    let reviewed_schedule = storage.commit_review(&review).unwrap();
    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    assert_eq!(storage.media_reference_usage("media-library").unwrap(), 1);

    storage
        .set_deck_cards_deleted(&[SAMPLE_CARD_ID.into()], Some(20_000), 20_000)
        .unwrap();
    let deleted = storage.library_notes().unwrap();
    assert_eq!(deleted[0].deleted_at_ms, Some(20_000));
    assert!(
        storage
            .study_cards_for_deck(DEFAULT_DECK_ID)
            .unwrap()
            .is_empty()
    );
    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    assert_eq!(
        storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
        reviewed_schedule
    );
    assert_eq!(storage.media_reference_usage("media-library").unwrap(), 1);
    assert_eq!(
        storage.get_media_reference("media-library").unwrap().id,
        "media-library"
    );

    storage
        .set_deck_cards_deleted(&[SAMPLE_CARD_ID.into()], None, 21_000)
        .unwrap();
    assert_eq!(
        storage.study_cards_for_deck(DEFAULT_DECK_ID).unwrap().len(),
        1
    );

    let missing_selection = vec![SAMPLE_CARD_ID.into(), "missing-card".into()];
    assert!(matches!(
        storage.set_deck_cards_suspended(&missing_selection, true, 22_000),
        Err(StorageError::EntityNotFound { entity: "card", .. })
    ));
    assert!(!storage.get_card(SAMPLE_CARD_ID).unwrap().suspended);

    storage
        .set_deck_cards_suspended(&[SAMPLE_CARD_ID.into()], true, 23_000)
        .unwrap();
    assert!(storage.get_card(SAMPLE_CARD_ID).unwrap().suspended);
    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    assert_eq!(
        storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
        reviewed_schedule
    );

    let destination = deck("library-destination");
    storage.create_deck(&destination).unwrap();
    storage
        .move_deck_cards(&[SAMPLE_CARD_ID.into()], &destination.id, 24_000)
        .unwrap();
    assert_eq!(
        storage
            .get_source_note(&source_id)
            .unwrap()
            .source_item
            .deck_id,
        destination.id
    );

    assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    assert_eq!(
        storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
        reviewed_schedule
    );
}

#[test]
#[ignore = "release performance budget; run with scripts/performance"]
fn release_budget_startup_and_current_schema_migration() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("startup.db");

    let migration_started = std::time::Instant::now();
    drop(Storage::open(&path).unwrap());
    let migration_elapsed = migration_started.elapsed();

    let startup_started = std::time::Instant::now();
    for _ in 0..50 {
        drop(Storage::open(&path).unwrap());
    }
    let startup_elapsed = startup_started.elapsed();

    assert!(
        migration_elapsed <= std::time::Duration::from_secs(2),
        "new collection migration exceeded 2 s: {migration_elapsed:?}"
    );
    assert!(
        startup_elapsed <= std::time::Duration::from_secs(5),
        "50 current-schema opens exceeded 5 s: {startup_elapsed:?}"
    );
    eprintln!(
        "release-budget migration_new_ms={} startup_50_ms={}",
        migration_elapsed.as_millis(),
        startup_elapsed.as_millis()
    );
}

#[test]
#[ignore = "release performance budget; run with scripts/performance"]
fn release_budget_large_version_eight_migration() {
    const CARD_COUNT: u32 = 10_000;
    let directory = tempdir().unwrap();
    let path = directory.path().join("large-v8.db");
    let mut legacy = open_version_eight_fixture(&path);
    legacy
        .seed_large_performance_fixture(CARD_COUNT, 1_000_000_000)
        .unwrap();
    assert_eq!(legacy.schema_version().unwrap(), 8);
    drop(legacy);
    let before_bytes = std::fs::metadata(&path).unwrap().len();

    let started = std::time::Instant::now();
    let migrated = Storage::open(&path).unwrap();
    let migration_elapsed = started.elapsed();
    let integrity = migrated.check_collection_schedule_integrity().unwrap();
    let after_bytes = std::fs::metadata(&path).unwrap().len();

    assert_eq!(migrated.schema_version().unwrap(), 13);
    assert_eq!(
        integrity.checked_cards,
        usize::try_from(CARD_COUNT).unwrap()
    );
    assert!(integrity.is_valid());
    assert_eq!(
        migration_backup_schema_version(directory.path(), "large-v8.db.migration-v8-"),
        8
    );
    assert!(
        migration_elapsed <= std::time::Duration::from_secs(60),
        "10,000-card schema-8 migration exceeded 60 s: {migration_elapsed:?}"
    );
    eprintln!(
        "release-budget migration_v8_10000 before_bytes={before_bytes} \
         after_bytes={after_bytes} elapsed_ms={}",
        migration_elapsed.as_millis()
    );
}

impl Storage {
    fn rebuildable_baseline_for_test(&self, card_id: &str) -> Result<ScheduleState, StorageError> {
        super::repository::load_schedule_row(&self.connection, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))
    }
}
