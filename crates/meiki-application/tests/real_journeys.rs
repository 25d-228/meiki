use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicU64, Ordering},
    },
};

use meiki_application::{
    AnnotationDraftDto, ApplicationRuntime, ApplicationService, ArchiveExportRequest,
    ArchiveImportRequest, CheckAnswerRequest, Clock, ComparisonResultDto, CreateDeckRequest,
    DirectionDto, GradeDto, GradeReviewRequest, IdSource, ImportMediaRequest, MakeClozeRequest,
    MatchingPolicyDto, MediaAvailabilityDto, MediaRoleDto, SchedulingModeDto, StudyAvailabilityDto,
    TodayRequest, UndoReviewRequest, UpdateSchedulerSettingsRequest,
};
use meiki_storage::Storage;
use tempfile::{TempDir, tempdir};

const NOW_MS: i64 = 1_700_000_000_000;
const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug)]
struct FixedClock(Arc<AtomicI64>);

impl FixedClock {
    fn new(now_ms: i64) -> Self {
        Self(Arc::new(AtomicI64::new(now_ms)))
    }

    fn set(&self, now_ms: i64) {
        self.0.store(now_ms, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug, Default)]
struct SequentialIds(Arc<AtomicU64>);

impl IdSource for SequentialIds {
    fn next_id(&self, purpose: &'static str) -> String {
        let sequence = self.0.fetch_add(1, Ordering::SeqCst);
        format!("{purpose}-{sequence:04}")
    }
}

struct AuthoredCollection {
    _directory: TempDir,
    collection_path: PathBuf,
    clock: FixedClock,
    ids: SequentialIds,
    service: ApplicationService,
    deck_id: String,
    card_id: String,
    media_hash: String,
}

impl AuthoredCollection {
    fn restart(&mut self) {
        self.service = service(&self.collection_path, &self.clock, &self.ids);
    }
}

fn service(path: &Path, clock: &FixedClock, ids: &SequentialIds) -> ApplicationService {
    ApplicationService::with_runtime(path, ApplicationRuntime::new(clock.clone(), ids.clone()))
}

fn today(deck_id: &str, now_ms: i64) -> TodayRequest {
    TodayRequest {
        deck_id: deck_id.to_owned(),
        now_ms,
        day_start_ms: now_ms - DAY_MS / 2,
        day_end_ms: now_ms + DAY_MS / 2,
    }
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![
        0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
    ];
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0]);
    bytes
}

fn utf16_selection(text: &str, selected: &str) -> (u32, u32) {
    let byte_start = text.find(selected).expect("fixture selection exists");
    let start = text[..byte_start].encode_utf16().count();
    let end = start + selected.encode_utf16().count();
    (
        u32::try_from(start).expect("fixture start fits the contract"),
        u32::try_from(end).expect("fixture end fits the contract"),
    )
}

fn authored_collection() -> AuthoredCollection {
    let directory = tempdir().expect("create journey directory");
    let collection_path = directory.path().join("collection.db");
    let clock = FixedClock::new(NOW_MS);
    let ids = SequentialIds::default();
    let application = service(&collection_path, &clock, &ids);

    let empty = application
        .prepare_study(&today("__all_decks__", NOW_MS))
        .expect("open a clean collection");
    assert_eq!(empty.availability, StudyAvailabilityDto::EmptyCollection);

    let deck = application
        .create_deck(&CreateDeckRequest {
            name: "Multilingual journey".into(),
            now_ms: NOW_MS,
        })
        .expect("create a real deck");
    let settings = application
        .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
            deck_id: deck.id.clone(),
            scheduling_mode: SchedulingModeDto::Expert,
            collection_daily_time_budget_minutes: 7,
            deck_daily_time_budget_minutes: None,
            target_retention_basis_points: 9_000,
            new_cards_per_day: 1,
            maximum_interval_days: 36_500,
            day_boundary_minutes: 240,
            now_ms: NOW_MS,
            day_start_ms: NOW_MS - DAY_MS / 2,
        })
        .expect("set the fixed journey budget");
    assert_eq!(settings.effective_daily_time_budget_minutes, 7);

    let source = "東京でCafe\u{301}を学ぶ — مرحبًا 👩🏽‍💻";
    let selected = "Cafe\u{301}";
    let mut draft = application
        .new_authoring_draft()
        .expect("start a real authoring draft");
    draft.deck_id.clone_from(&deck.id);
    draft.language_tag = Some("ja".into());
    draft.direction = DirectionDto::Auto;
    draft.deck_matching_policy = MatchingPolicyDto::Strict;
    draft.segments[0].text = source.into();
    let segment_id = draft.segments[0].id.clone();
    let (selection_start_utf16, selection_end_utf16) = utf16_selection(source, selected);
    let mut draft = application
        .make_cloze(MakeClozeRequest {
            draft,
            segment_id,
            selection_start_utf16,
            selection_end_utf16,
        })
        .expect("make a cloze on complete grapheme boundaries");

    let media_fixture = directory.path().join("東京.png");
    fs::write(&media_fixture, png(320, 180)).expect("write media fixture");
    let media = application
        .import_media(&ImportMediaRequest {
            path: media_fixture.to_string_lossy().into_owned(),
            role: MediaRoleDto::RevealImage,
            language_tag: Some("ja".into()),
            direction: DirectionDto::Auto,
        })
        .expect("import through the real media store");

    draft.clozes[0].accepted_answers = vec!["coffee".into()];
    draft.clozes[0].hint = "A borrowed word".into();
    draft.clozes[0].annotations = vec![AnnotationDraftDto {
        id: "annotation-meaning".into(),
        label: "العربية".into(),
        value: "قهوة".into(),
        language_tag: Some("ar".into()),
        direction: DirectionDto::Rtl,
    }];
    draft.clozes[0].explanation_markdown = "**Café** is preserved as one grapheme.".into();
    draft.clozes[0].media = vec![media.clone()];
    let previews = application
        .preview_authoring_draft(&draft)
        .expect("render the real text projection");
    assert_eq!(previews[0].answer, selected);
    assert!(previews[0].prompt.contains("[…]"));

    let saved = application
        .save_authoring_draft(&draft)
        .expect("save through real SQLite");
    let card_id = saved.clozes[0].card_id.clone();

    let mut fixture = AuthoredCollection {
        _directory: directory,
        collection_path,
        clock,
        ids,
        service: application,
        deck_id: deck.id,
        card_id,
        media_hash: media.content_hash,
    };
    fixture.restart();
    fixture
}

fn accepted_reveal(fixture: &AuthoredCollection) -> meiki_application::RevealDto {
    let card = fixture
        .service
        .get_study_card(&fixture.card_id)
        .expect("load the persisted card");
    let reveal = fixture
        .service
        .check_answer(&CheckAnswerRequest {
            card_id: fixture.card_id.clone(),
            card_content_version: card.card_content_version,
            schedule_version: card.schedule_version,
            raw_response: "coffee".into(),
        })
        .expect("compare through the real text engine");
    assert_eq!(reveal.comparison, ComparisonResultDto::AcceptedVariant);
    reveal
}

fn grade_request(
    fixture: &AuthoredCollection,
    event_id: &str,
    grade: GradeDto,
) -> GradeReviewRequest {
    let card = fixture
        .service
        .get_study_card(&fixture.card_id)
        .expect("load the current card");
    GradeReviewRequest {
        review_event_id: event_id.into(),
        card_id: fixture.card_id.clone(),
        card_content_version: card.card_content_version,
        schedule_version: card.schedule_version,
        raw_response: "coffee".into(),
        chosen_grade: grade,
        response_duration_ms: 1_250,
    }
}

fn grade_retry_undo_and_regrade(fixture: &mut AuthoredCollection) {
    let initial = fixture
        .service
        .get_study_card(&fixture.card_id)
        .expect("load the initial schedule");
    let request = grade_request(fixture, "review-idempotent", GradeDto::Good);

    let lost_response = fixture
        .service
        .grade_review(&request)
        .expect("commit before the simulated response loss");
    let retry = fixture
        .service
        .grade_review(&request)
        .expect("retry the same idempotency identity");
    assert_eq!(retry.review_event_id, lost_response.review_event_id);
    assert_eq!(retry.schedule_version, lost_response.schedule_version);
    assert_eq!(
        Storage::open(&fixture.collection_path)
            .expect("open the collection for invariant inspection")
            .review_events(&fixture.card_id)
            .expect("load immutable history")
            .len(),
        1
    );

    let undone = fixture
        .service
        .undo_review(&UndoReviewRequest {
            undo_event_id: "undo-idempotent".into(),
            card_id: fixture.card_id.clone(),
            card_content_version: initial.card_content_version,
            schedule_version: retry.schedule_version,
            review_event_id: request.review_event_id,
        })
        .expect("append a compensating undo event");
    assert_eq!(undone.completed_reviews, 0);
    assert_eq!(undone.due_at, initial.due_at);

    let regrade = grade_request(fixture, "review-after-undo", GradeDto::Easy);
    fixture
        .service
        .grade_review(&regrade)
        .expect("continue after undo");
    fixture.restart();
    assert_eq!(
        Storage::open(&fixture.collection_path)
            .expect("reopen SQLite after restart")
            .review_events(&fixture.card_id)
            .expect("load review, undo, and regrade events")
            .len(),
        3
    );
}

fn continue_study_in_lockstep(
    source: &mut AuthoredCollection,
    target: &ApplicationService,
    target_path: &Path,
    target_clock: &FixedClock,
    due_ms: i64,
) {
    source.clock.set(due_ms);
    target_clock.set(due_ms);
    let source_plan = source
        .service
        .prepare_study(&today(&source.deck_id, due_ms))
        .expect("continue source study at the exact due timestamp");
    let target_plan = target
        .prepare_study(&today(&source.deck_id, due_ms))
        .expect("continue imported study at the exact due timestamp");
    assert_eq!(target_plan, source_plan);

    let request = GradeReviewRequest {
        review_event_id: "review-after-import".into(),
        card_id: source.card_id.clone(),
        card_content_version: source_plan.overview.queue[0].card_content_version,
        schedule_version: source_plan.overview.queue[0].schedule_version,
        raw_response: "coffee".into(),
        chosen_grade: GradeDto::Good,
        response_duration_ms: 1_500,
    };
    let source_result = source
        .service
        .grade_review(&request)
        .expect("continue the source history");
    let target_result = target
        .grade_review(&request)
        .expect("continue the imported history");
    assert_eq!(
        target_result.schedule_version,
        source_result.schedule_version
    );
    assert_eq!(target_result.due_at, source_result.due_at);
    assert_eq!(
        Storage::open(target_path)
            .expect("reopen imported collection after continuation")
            .review_events(&source.card_id)
            .expect("load continued imported history"),
        Storage::open(&source.collection_path)
            .expect("reopen source collection after continuation")
            .review_events(&source.card_id)
            .expect("load continued source history")
    );
}

#[test]
fn authoring_restart_reaches_the_exact_due_card_through_real_boundaries() {
    let fixture = authored_collection();
    let draft = fixture
        .service
        .get_authoring_draft_for_card(&fixture.card_id)
        .expect("reload source, cloze, annotations, and media");
    assert!(draft.persisted);
    assert_eq!(draft.language_tag.as_deref(), Some("ja"));
    assert_eq!(draft.clozes[0].accepted_answers, ["coffee"]);
    assert_eq!(draft.clozes[0].annotations[0].direction, DirectionDto::Rtl);
    assert_eq!(
        draft.clozes[0].media[0].availability,
        MediaAvailabilityDto::Ready
    );
    assert_eq!(draft.clozes[0].media[0].content_hash, fixture.media_hash);

    let plan = fixture
        .service
        .prepare_study(&today(&fixture.deck_id, NOW_MS))
        .expect("build Today through the real scheduler");
    assert_eq!(plan.availability, StudyAvailabilityDto::Ready);
    assert_eq!(plan.overview.daily_time_budget_minutes, Some(7));
    assert_eq!(plan.overview.queue.len(), 1);
    assert_eq!(plan.overview.queue[0].card_id, fixture.card_id);
    let card = fixture
        .service
        .get_study_card(&fixture.card_id)
        .expect("load the exact due card");
    assert_eq!(plan.overview.queue[0].due_at, card.due_at);
    let reveal = accepted_reveal(&fixture);
    assert_eq!(reveal.annotations[0].value, "قهوة");
    assert_eq!(
        reveal.answer_media[0].availability,
        MediaAvailabilityDto::Ready
    );
}

#[test]
fn response_loss_retry_is_exactly_once_and_undo_is_compensating() {
    let mut fixture = authored_collection();
    accepted_reveal(&fixture);
    grade_retry_undo_and_regrade(&mut fixture);

    let stored = Storage::open(&fixture.collection_path)
        .expect("open persisted collection")
        .load_study_card(&fixture.card_id)
        .expect("load persisted lifecycle");
    let events = Storage::open(&fixture.collection_path)
        .expect("open persisted collection")
        .review_events(&fixture.card_id)
        .expect("load immutable review history");
    assert_eq!(stored.schedule.version, 3);
    assert_eq!(events[0].id, "review-idempotent");
    assert_eq!(events[1].id, "undo-idempotent");
    assert_eq!(events[2].id, "review-after-undo");
    assert_eq!(
        events[1].undoes_review_event_id.as_deref(),
        Some("review-idempotent")
    );
}

#[test]
fn full_archive_replacement_preserves_history_media_and_continued_study() {
    let mut source = authored_collection();
    grade_retry_undo_and_regrade(&mut source);
    let source_stored = Storage::open(&source.collection_path)
        .expect("open source collection")
        .load_study_card(&source.card_id)
        .expect("load source aggregate");
    let source_history = Storage::open(&source.collection_path)
        .expect("open source collection")
        .review_events(&source.card_id)
        .expect("load source history");
    let exported = source
        .service
        .export_archive(&ArchiveExportRequest { now_ms: NOW_MS + 1 })
        .expect("export the complete production archive");
    assert_eq!(exported.cards, 1);
    assert_eq!(exported.review_events, 3);
    assert_eq!(exported.media_objects, 1);
    let preview = source
        .service
        .preview_archive(&exported.path)
        .expect("validate the archive before replacement");
    assert!(preview.can_import);

    let target_directory = tempdir().expect("create replacement target");
    let target_path = target_directory.path().join("collection.db");
    let target_clock = FixedClock::new(NOW_MS);
    let target_ids = SequentialIds::default();
    let target = service(&target_path, &target_clock, &target_ids);
    assert_eq!(
        target
            .prepare_study(&today("__all_decks__", NOW_MS))
            .expect("open a clean replacement target")
            .availability,
        StudyAvailabilityDto::EmptyCollection
    );
    let imported = target
        .import_archive(&ArchiveImportRequest {
            path: exported.path,
            confirmation: "REPLACE".into(),
        })
        .expect("replace through staging and a recovery backup");
    assert_eq!(imported.imported_cards, 1);
    assert_eq!(imported.imported_media_objects, 1);

    let target_stored = Storage::open(&target_path)
        .expect("open replaced collection")
        .load_study_card(&source.card_id)
        .expect("load replaced aggregate");
    let target_history = Storage::open(&target_path)
        .expect("open replaced collection")
        .review_events(&source.card_id)
        .expect("load replaced history");
    assert_eq!(target_stored, source_stored);
    assert_eq!(target_history, source_history);
    let target_draft = target
        .get_authoring_draft_for_card(&source.card_id)
        .expect("load imported media references");
    assert_eq!(
        target_draft.clozes[0].media[0].availability,
        MediaAvailabilityDto::Ready
    );
    assert_eq!(
        target_draft.clozes[0].media[0].content_hash,
        source.media_hash
    );

    continue_study_in_lockstep(
        &mut source,
        &target,
        &target_path,
        &target_clock,
        target_stored.schedule.due_at_ms,
    );
}
