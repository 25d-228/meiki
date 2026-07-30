use meiki_domain::{
    Annotation, Card, CardLifecycle, Cloze, ComparisonResult, Deck, Direction, Grade,
    LocalizedText, MatchingPolicy, MediaKind, MediaReference, MediaRole, ReviewEvent,
    ReviewEventKind, ScheduleState, SchedulerParameterSet, SegmentContent, SemanticSegment,
    SourceItem, StudySettingsOverride, Tag,
};
use rusqlite::Connection;
use tempfile::tempdir;

use super::{
    AUTHORING_DEFAULTS_MIGRATION, AnnotationRepository, CARD_LIFECYCLE_MIGRATION,
    CORE_MODEL_MIGRATION, CardRepository, ClozeRepository, DEFAULT_DECK_ID, DeckRepository,
    FOUNDATION_MIGRATION, FSRS7_SCHEDULER_MIGRATION, LIBRARY_MIGRATION, MEDIA_PIPELINE_MIGRATION,
    MediaRepository, PROJECTION_INTEGRITY_MIGRATION, SAMPLE_CARD_ID, STUDY_SESSION_MIGRATION,
    SchedulerParameterSetRepository, SchedulerProfileRepository, SourceNoteRepository, Storage,
    StorageError, StoredSourceNote, TagRepository,
};

fn sample_event(storage: &Storage, id: &str, reviewed_at_ms: i64) -> ReviewEvent {
    let stored = storage.load_study_card(SAMPLE_CARD_ID).unwrap();
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
fn parameter_adoption_and_rollback_are_atomic_and_prospective() {
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
        .adopt_scheduler_parameter_set(
            DEFAULT_DECK_ID,
            &personalized,
            "{\"result\":\"adopted\",\"reviews\":64}",
            2_000,
        )
        .unwrap();
    assert_eq!(adopted.active_parameter_set_id, personalized.id);
    assert_eq!(
        adopted.previous_parameter_set_id.as_deref(),
        Some(default_profile.active_parameter_set_id.as_str())
    );
    assert_eq!(
        adopted.optimizer_status,
        meiki_domain::OptimizerStatus::Adopted
    );
    assert_eq!(
        storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
        schedule_before
    );
    assert_eq!(
        storage.review_count(SAMPLE_CARD_ID).unwrap(),
        history_before
    );

    let rolled_back = storage
        .rollback_scheduler_parameter_set(DEFAULT_DECK_ID, 3_000)
        .unwrap();
    assert_eq!(
        rolled_back.active_parameter_set_id,
        default_profile.active_parameter_set_id
    );
    assert_eq!(
        rolled_back.previous_parameter_set_id.as_deref(),
        Some(personalized.id.as_str())
    );
    assert_eq!(
        rolled_back.optimizer_status,
        meiki_domain::OptimizerStatus::RolledBack
    );
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
fn released_v0_1_schema_fixture_opens_and_migrates() {
    const RELEASED_V0_1_SCHEMA: &[u8] = include_bytes!("../fixtures/released/v0.1-schema-7.db");
    let directory = tempdir().unwrap();
    let path = directory.path().join("released-v0.1.db");
    std::fs::write(&path, RELEASED_V0_1_SCHEMA).unwrap();

    let storage = Storage::open(&path).unwrap();
    assert_eq!(storage.schema_version().unwrap(), 10);
    assert_eq!(storage.get_deck(DEFAULT_DECK_ID).unwrap().name, "Default");
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
    assert_eq!(migrated.schema_version().unwrap(), 10);
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
    assert_eq!(repaired.schema_version().unwrap(), 10);
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
    assert_eq!(storage.schema_version().unwrap(), 10);
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
    assert_eq!(storage.schema_version().unwrap(), 10);
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
            settings: StudySettingsOverride::default(),
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

#[test]
#[allow(clippy::too_many_lines)]
fn library_bulk_actions_are_recoverable_atomic_and_preserve_history_and_media() {
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
        .set_library_notes_deleted(std::slice::from_ref(&source_id), Some(20_000), 20_000)
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
        .set_library_notes_deleted(std::slice::from_ref(&source_id), None, 21_000)
        .unwrap();
    assert_eq!(
        storage.study_cards_for_deck(DEFAULT_DECK_ID).unwrap().len(),
        1
    );

    let missing_selection = vec![source_id.clone(), "missing-source".into()];
    assert!(matches!(
        storage.set_library_notes_suspended(&missing_selection, true, 22_000),
        Err(StorageError::EntityNotFound {
            entity: "source note",
            ..
        })
    ));
    assert!(!storage.get_card(SAMPLE_CARD_ID).unwrap().suspended);

    storage
        .set_library_notes_suspended(std::slice::from_ref(&source_id), true, 23_000)
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
        .move_library_notes(std::slice::from_ref(&source_id), &destination.id, 24_000)
        .unwrap();
    assert_eq!(
        storage
            .get_source_note(&source_id)
            .unwrap()
            .source_item
            .deck_id,
        destination.id
    );

    let library_tag = tag("tag-library", "検索");
    storage
        .tag_library_notes(std::slice::from_ref(&source_id), &library_tag, 25_000)
        .unwrap();
    assert!(
        storage
            .get_source_note(&source_id)
            .unwrap()
            .source_item
            .tags
            .iter()
            .any(|stored| stored.id == library_tag.id)
    );
    storage
        .untag_library_notes(std::slice::from_ref(&source_id), &library_tag.id, 26_000)
        .unwrap();
    assert!(
        storage
            .get_source_note(&source_id)
            .unwrap()
            .source_item
            .tags
            .iter()
            .all(|stored| stored.id != library_tag.id)
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

impl Storage {
    fn rebuildable_baseline_for_test(&self, card_id: &str) -> Result<ScheduleState, StorageError> {
        super::repository::load_schedule_row(&self.connection, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))
    }
}
