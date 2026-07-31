use std::{
    env, fs,
    path::{Path, PathBuf},
};

use meiki_application::{ApplicationService, ArchiveExportRequest};
use meiki_domain::{
    Card, CardLifecycle, Cloze, CollectionSchedulingSettings, Deck, Direction, MatchingPolicy,
    MediaKind, MediaReference, MediaRole, ScheduleState, SegmentContent, SemanticSegment,
    SourceItem, StudySettingsOverride,
};
use meiki_media::{ImportedMedia, MediaStore};
use meiki_storage::{
    CardRepository, DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository,
    SchedulerParameterSetRepository, SourceNoteRepository, Storage, StoredSourceNote,
};
use tempfile::tempdir;

const FIXTURE_DECK_ID: &str = "fixture-deck-ja-foundation";

fn main() {
    let destination = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: generate_pristine_deck_fixture DESTINATION");
    assert!(
        !destination.exists(),
        "fixture destination already exists: {}",
        destination.display()
    );
    generate_fixture(&destination);
    println!("{}", destination.display());
}

fn generate_fixture(destination: &Path) {
    let temporary = tempdir().expect("create fixture workspace");
    let collection_path = temporary.path().join("collection.db");
    let wav_path = temporary.path().join("fixture.wav");
    fs::write(&wav_path, wav_bytes()).expect("write fixture audio");
    let media_store = MediaStore::new(collection_path.with_extension("media"));
    let imported = media_store
        .import_file(&wav_path)
        .expect("import fixture audio");

    let mut storage = Storage::open(&collection_path).expect("open fixture collection");
    storage
        .update_collection_scheduling_settings(&CollectionSchedulingSettings {
            daily_time_budget_minutes: 30,
            updated_at_ms: 0,
        })
        .expect("pin fixture collection settings");
    let mut default_parameters = storage
        .get_scheduler_parameter_set(DEFAULT_SCHEDULER_PARAMETER_SET_ID)
        .expect("load default scheduler parameters");
    storage
        .delete_deck(DEFAULT_DECK_ID)
        .expect("remove unused default deck");
    storage
        .delete_scheduler_parameter_set(DEFAULT_SCHEDULER_PARAMETER_SET_ID)
        .expect("remove timestamped default scheduler parameters");
    default_parameters.created_at_ms = 0;
    storage
        .create_scheduler_parameter_set(&default_parameters)
        .expect("recreate deterministic default scheduler parameters");
    storage
        .create_deck(&Deck {
            id: FIXTURE_DECK_ID.into(),
            name: "Japanese Foundation Fixture".into(),
            description: Some("One-card pristine archive test fixture".into()),
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
            matching_policy: MatchingPolicy::Strict,
            settings: StudySettingsOverride::default(),
            created_at_ms: 0,
            updated_at_ms: 0,
        })
        .expect("create fixture deck");
    storage
        .create_source_note(&fixture_note(&imported))
        .expect("create fixture note");
    let (card, initial_schedule) = fixture_card();
    storage
        .create_card(&card, &initial_schedule)
        .expect("create fixture card");
    drop(storage);

    let exported = ApplicationService::new(&collection_path)
        .export_archive(&ArchiveExportRequest { now_ms: 0 })
        .expect("export fixture archive");
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).expect("create fixture destination directory");
    }
    fs::rename(exported.path, destination).expect("move generated fixture");
}

fn fixture_note(imported: &ImportedMedia) -> StoredSourceNote {
    StoredSourceNote {
        source_item: SourceItem {
            id: "fixture-source-ja-001".into(),
            deck_id: FIXTURE_DECK_ID.into(),
            segments: vec![
                SemanticSegment {
                    id: "fixture-segment-ja-001-context".into(),
                    ordinal: 0,
                    content: SegmentContent::Text("今日は".into()),
                },
                SemanticSegment {
                    id: "fixture-segment-ja-001-cloze".into(),
                    ordinal: 1,
                    content: SegmentContent::Cloze {
                        cloze_id: "fixture-cloze-ja-001".into(),
                        text: "晴れです".into(),
                    },
                },
            ],
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
            tags: Vec::new(),
            annotations: Vec::new(),
            explanation: None,
            media: vec![
                media_reference("fixture-prompt-ja-001", MediaRole::PromptAudio, imported),
                media_reference("fixture-answer-ja-001", MediaRole::AnswerAudio, imported),
            ],
            created_at_ms: 0,
            updated_at_ms: 0,
        },
        clozes: vec![Cloze {
            id: "fixture-cloze-ja-001".into(),
            source_item_id: "fixture-source-ja-001".into(),
            answer: "晴れです".into(),
            accepted_answers: Vec::new(),
            hint: None,
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
            matching_policy: Some(MatchingPolicy::Strict),
            annotations: Vec::new(),
            explanation: None,
            media: Vec::new(),
            created_at_ms: 0,
            updated_at_ms: 0,
        }],
    }
}

fn media_reference(id: &str, role: MediaRole, imported: &ImportedMedia) -> MediaReference {
    MediaReference {
        id: id.into(),
        content_hash: imported.content_hash.clone(),
        kind: MediaKind::Audio,
        role,
        media_type: imported.media_type.clone(),
        byte_size: imported.byte_size,
        original_file_name: Some("fixture.wav".into()),
        alt_text: None,
        width: None,
        height: None,
        duration_ms: imported.duration_ms,
        language_tag: Some("ja".into()),
        direction: Direction::Auto,
        created_at_ms: 0,
    }
}

fn fixture_card() -> (Card, ScheduleState) {
    let card = Card {
        id: "fixture-card-ja-001".into(),
        cloze_id: "fixture-cloze-ja-001".into(),
        content_version: 1,
        suspended: false,
        created_at_ms: 0,
        updated_at_ms: 0,
    };
    let schedule = ScheduleState {
        card_id: card.id.clone(),
        version: 0,
        lifecycle: CardLifecycle::Unseen,
        due_at_ms: 0,
        ideal_due_at_ms: 0,
        interval_milliseconds: 0,
        interval_seconds: 0,
        repetitions: 0,
        stability_milliseconds: 0,
        difficulty_millipoints: 0,
        last_reviewed_at_ms: None,
        last_review_event_id: None,
    };
    (card, schedule)
}

fn wav_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend(b"RIFF");
    bytes.extend(36_u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(8_000_u32.to_le_bytes());
    bytes.extend(16_000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(0_u32.to_le_bytes());
    bytes
}
