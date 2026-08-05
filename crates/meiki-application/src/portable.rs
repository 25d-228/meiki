use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use crate::{ApplicationError, ApplicationService};
use meiki_domain::{
    CardLifecycle, MediaKind, MediaReference, ScheduleState, SchedulingMode, StudySettingsOverride,
};
use meiki_portable::{
    ArchiveMediaSource, ArchivePreview, PortableCard, PortableCollection, PortableNote,
    ValidatedArchive, read_archive, read_archive_preview, write_archive,
};
use meiki_scheduler::{BASELINE_TARGET_RETENTION_BASIS_POINTS, CONTROLLER_VERSION};
use meiki_storage::{
    DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository, PristineBundleImport,
    PristineBundleImportError, PristineBundleImportPlan, PristineDeckCard, PristineDeckImport,
    PristineDeckNote, SchedulerParameterSetRepository, SchedulerProfileRepository, Storage,
    StorageError, StoredSourceNote,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const BACKUP_RETENTION: usize = 5;
const BUNDLE_DEFAULT_NEW_CARDS_PER_DAY: u32 = 20;
const BUNDLE_DEFAULT_DAY_BOUNDARY_MINUTES: u16 = 240;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct PortableExportResultDto {
    pub path: String,
    #[ts(type = "number")]
    pub decks: u64,
    #[ts(type = "number")]
    pub notes: u64,
    #[ts(type = "number")]
    pub cards: u64,
    #[ts(type = "number")]
    pub review_events: u64,
    #[ts(type = "number")]
    pub media_objects: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum BundleDeckInstallStatusDto {
    Installed,
    Missing,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleDeckPreviewDto {
    pub id: String,
    pub name: String,
    #[ts(type = "number")]
    pub cards: u64,
    pub status: BundleDeckInstallStatusDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundlePreviewDto {
    pub path: String,
    pub language_tag: String,
    pub decks: Vec<BundleDeckPreviewDto>,
    #[ts(type = "number")]
    pub total_cards: u64,
    #[ts(type = "number")]
    pub audio_objects: u64,
    pub can_import: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleImportRequest {
    pub path: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum BundleImportStageDto {
    PreparingDecks,
    AddingCards,
    AddingAudio,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleImportProgressDto {
    pub stage: BundleImportStageDto,
    #[ts(type = "number")]
    pub current: u64,
    #[ts(type = "number")]
    pub total: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleImportResultDto {
    pub language_tag: String,
    #[ts(type = "number")]
    pub added_decks: u64,
    #[ts(type = "number")]
    pub added_cards: u64,
    #[ts(type = "number")]
    pub imported_media_objects: u64,
    #[ts(type = "number")]
    pub deduplicated_media_objects: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BundleExportRequest {
    pub language_tag: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

struct MissingBundleContent {
    cards: u64,
    media_hashes: HashSet<String>,
    audio_hashes: HashSet<String>,
}

impl ApplicationService {
    /// Previews an installable language bundle without reading media payloads.
    ///
    /// # Errors
    ///
    /// Returns an error when archive metadata or collection relationships are
    /// invalid, the content is not a pristine single-language bundle, or a
    /// missing deck identity collides with the current collection.
    pub fn preview_bundle(&self, path: &str) -> Result<BundlePreviewDto, ApplicationError> {
        let archive = read_archive_preview(Path::new(path))?;
        preview_bundle_archive(self, path, &archive)
    }

    /// Adds every missing deck from one pristine language bundle.
    ///
    /// Full media checks run once before durable completion. Missing decks and
    /// associations are committed only after required media is safely present.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid timestamp, unsupported bundle content,
    /// identity collision, corrupt media, or failed transactional write.
    pub fn import_bundle(
        &self,
        request: &BundleImportRequest,
        on_progress: impl FnMut(BundleImportProgressDto),
    ) -> Result<BundleImportResultDto, ApplicationError> {
        if request.now_ms < 0 {
            return Err(ApplicationError::InvalidPortable(
                "bundle import timestamp is invalid".into(),
            ));
        }

        let fast_archive = read_archive_preview(Path::new(&request.path))?;
        let fast_import = build_pristine_bundle_import(&fast_archive.collection, request.now_ms)?;
        let fast_plan = self
            .open_storage()?
            .validate_pristine_bundle_import(&fast_import)?;
        if !fast_plan.requires_changes() {
            return Ok(bundle_import_result(&fast_import.language_tag));
        }
        let missing_decks = count(
            fast_plan.missing_deck_ids.len(),
            "missing bundle deck count",
        )?;
        let progress = RefCell::new(on_progress);
        (progress.borrow_mut())(BundleImportProgressDto {
            stage: BundleImportStageDto::PreparingDecks,
            current: 0,
            total: missing_decks,
        });

        let archive = read_archive(Path::new(&request.path))?;
        let bundle = build_pristine_bundle_import(&archive.collection, request.now_ms)?;
        let mut storage = self.open_storage()?;
        let plan = storage.validate_pristine_bundle_import(&bundle)?;
        if !plan.requires_changes() {
            return Ok(bundle_import_result(&bundle.language_tag));
        }
        let missing_content = missing_bundle_content(&archive.collection, &bundle, &plan)?;
        let added_decks = count(plan.missing_deck_ids.len(), "added bundle deck count")?;
        (progress.borrow_mut())(BundleImportProgressDto {
            stage: BundleImportStageDto::PreparingDecks,
            current: added_decks,
            total: added_decks,
        });

        self.create_recovery_backup(&storage, "pre-add-bundle")?;
        let mut imported_cards = 0_u64;
        (progress.borrow_mut())(BundleImportProgressDto {
            stage: BundleImportStageDto::AddingCards,
            current: 0,
            total: missing_content.cards,
        });
        let audio_total = count(missing_content.audio_hashes.len(), "bundle audio count")?;
        let mut imported_audio = 0_u64;
        let media_store = self.media_store();
        let mut staged_media_hashes = Vec::new();
        let imported = storage.import_pristine_bundle(
            &bundle,
            || {
                imported_cards += 1;
                (progress.borrow_mut())(BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingCards,
                    current: imported_cards,
                    total: missing_content.cards,
                });
            },
            || {
                (progress.borrow_mut())(BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingAudio,
                    current: 0,
                    total: audio_total,
                });
                let media_import = import_archive_media_subset(
                    &archive,
                    &media_store,
                    &missing_content.media_hashes,
                    |content_hash| {
                        if missing_content.audio_hashes.contains(content_hash) {
                            imported_audio += 1;
                            (progress.borrow_mut())(BundleImportProgressDto {
                                stage: BundleImportStageDto::AddingAudio,
                                current: imported_audio,
                                total: audio_total,
                            });
                        }
                        Ok(())
                    },
                )?;
                // A commit failure happens after media validation, so retain
                // exactly the new hashes that must be removed in that case.
                staged_media_hashes.clone_from(&media_import.new_hashes);
                Ok(media_import)
            },
        );
        let (imported_plan, media_import) = match imported {
            Ok(imported) => imported,
            Err(PristineBundleImportError::BeforeCommit(error)) => return Err(error),
            Err(PristineBundleImportError::Storage(error)) => {
                rollback_archive_media(&media_store, &staged_media_hashes)?;
                return Err(error.into());
            }
        };

        Ok(BundleImportResultDto {
            language_tag: bundle.language_tag,
            added_decks: count(imported_plan.missing_deck_ids.len(), "imported deck count")?,
            added_cards: missing_content.cards,
            imported_media_objects: media_import.imported,
            deduplicated_media_objects: media_import.deduplicated,
        })
    }

    /// Exports the remaining decks in one installed language bundle without
    /// personal study state.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, a bundle without remaining decks,
    /// inconsistent stored content, missing or corrupt media, or failed
    /// archive persistence.
    pub fn export_bundle(
        &self,
        request: &BundleExportRequest,
    ) -> Result<PortableExportResultDto, ApplicationError> {
        if request.language_tag.trim().is_empty() || request.now_ms < 0 {
            return Err(ApplicationError::InvalidPortable(
                "bundle export requires a language and a valid timestamp".into(),
            ));
        }
        let storage = self.open_storage()?;
        let collection = build_bundle_collection(&storage, &request.language_tag)?;
        let media = media_sources(&collection, &self.media_store())?;
        let directory = self.export_directory()?;
        let path = directory.join(format!(
            "meiki-bundle-{}-{}.meiki",
            request.now_ms,
            self.next_id("portable-bundle")
        ));
        let manifest = write_archive(&path, &collection, &media, request.now_ms)?;
        Ok(PortableExportResultDto {
            path: path.to_string_lossy().into_owned(),
            decks: manifest.counts.decks,
            notes: manifest.counts.notes,
            cards: manifest.counts.cards,
            review_events: manifest.counts.review_events,
            media_objects: manifest.counts.media_objects,
        })
    }

    fn export_directory(&self) -> Result<PathBuf, ApplicationError> {
        let directory = self
            .collection_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("exports");
        fs::create_dir_all(&directory).map_err(ApplicationError::PortableIo)?;
        Ok(directory)
    }

    fn backup_directory(&self) -> PathBuf {
        self.collection_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups")
    }

    pub(crate) fn create_recovery_backup(
        &self,
        storage: &Storage,
        reason: &str,
    ) -> Result<PathBuf, ApplicationError> {
        let backup =
            storage.create_rolling_backup(&self.collection_path, reason, BACKUP_RETENTION)?;
        self.media_store().backup_to(&media_backup_path(&backup))?;
        prune_orphan_media_backups(&self.backup_directory())?;
        Ok(backup)
    }
}

fn media_backup_path(database_backup: &Path) -> PathBuf {
    let file_name = database_backup
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("collection.bak");
    database_backup.with_file_name(format!("{file_name}.media"))
}

fn prune_orphan_media_backups(directory: &Path) -> Result<(), ApplicationError> {
    for entry in fs::read_dir(directory)
        .map_err(ApplicationError::PortableIo)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(ApplicationError::PortableIo)?
    {
        let path = entry.path();
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(database_name) = file_name.strip_suffix(".media") else {
            continue;
        };
        if path.is_dir() && !directory.join(database_name).is_file() {
            fs::remove_dir_all(&path).map_err(ApplicationError::PortableIo)?;
        }
    }
    Ok(())
}

fn build_bundle_collection(
    storage: &Storage,
    language_tag: &str,
) -> Result<PortableCollection, ApplicationError> {
    let deck_ids = storage.bundle_deck_ids(language_tag)?;
    if deck_ids.is_empty() {
        return Err(ApplicationError::InvalidPortable(format!(
            "No installed decks remain for {language_tag}."
        )));
    }
    let deck_id_set = deck_ids.iter().map(String::as_str).collect::<HashSet<_>>();
    let mut decks = deck_ids
        .iter()
        .map(|deck_id| storage.get_deck(deck_id))
        .collect::<Result<Vec<_>, _>>()?;
    for deck in &mut decks {
        deck.settings = StudySettingsOverride::default();
    }

    let mut notes = storage
        .library_notes()?
        .into_iter()
        .filter(|note| {
            note.deleted_at_ms.is_none()
                && deck_id_set.contains(note.note.source_item.deck_id.as_str())
        })
        .map(|stored| {
            let mut cards = stored
                .cards
                .into_iter()
                .map(|stored_card| {
                    let mut card = stored_card.card;
                    card.suspended = false;
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
                    PortableCard {
                        card,
                        baseline: schedule.clone(),
                        schedule,
                        review_events: Vec::new(),
                    }
                })
                .collect::<Vec<_>>();
            cards.sort_by(|left, right| left.card.id.cmp(&right.card.id));
            let mut clozes = stored.note.clozes;
            clozes.sort_by(|left, right| left.id.cmp(&right.id));
            PortableNote {
                source_item: stored.note.source_item,
                clozes,
                cards,
                deleted_at_ms: None,
            }
        })
        .collect::<Vec<_>>();
    notes.sort_by(|left, right| left.source_item.id.cmp(&right.source_item.id));

    let scheduler_parameter_set =
        storage.get_scheduler_parameter_set(DEFAULT_SCHEDULER_PARAMETER_SET_ID)?;
    let scheduler_profiles = deck_ids
        .iter()
        .map(|deck_id| {
            let mut profile = storage.get_scheduler_profile(deck_id)?;
            profile
                .engine_version
                .clone_from(&scheduler_parameter_set.engine_version);
            profile.active_parameter_set_id = DEFAULT_SCHEDULER_PARAMETER_SET_ID.into();
            profile.scheduling_mode = SchedulingMode::Automatic;
            profile.deck_daily_time_budget_minutes = None;
            profile.controller_version = CONTROLLER_VERSION.into();
            profile.controller_target_retention_basis_points =
                BASELINE_TARGET_RETENTION_BASIS_POINTS;
            profile.controller_new_cards_per_day = BUNDLE_DEFAULT_NEW_CARDS_PER_DAY;
            profile.controller_last_evaluated_day_start_ms = None;
            profile.controller_review_count = 0;
            profile.controller_unseen_count = 0;
            profile.controller_forecast_review_seconds_per_day = 0;
            profile.controller_backlog_exceeds_budget = false;
            profile.controller_explanation.clear();
            profile.day_boundary_minutes = BUNDLE_DEFAULT_DAY_BOUNDARY_MINUTES;
            profile.updated_at_ms = 0;
            Ok(profile)
        })
        .collect::<Result<Vec<_>, StorageError>>()?;

    Ok(PortableCollection {
        collection_scheduling_settings: meiki_domain::CollectionSchedulingSettings::default(),
        decks,
        notes,
        scheduler_parameter_sets: vec![scheduler_parameter_set],
        scheduler_profiles,
    })
}

fn media_sources(
    collection: &PortableCollection,
    store: &meiki_media::MediaStore,
) -> Result<Vec<ArchiveMediaSource>, ApplicationError> {
    let mut hashes = HashSet::new();
    for media in collection.notes.iter().flat_map(|note| {
        note.source_item
            .media
            .iter()
            .chain(note.clozes.iter().flat_map(|cloze| cloze.media.iter()))
    }) {
        hashes.insert(media.content_hash.clone());
    }
    let mut sources = hashes
        .into_iter()
        .map(|content_hash| {
            let path = store.resolve(&content_hash)?;
            Ok(ArchiveMediaSource { content_hash, path })
        })
        .collect::<Result<Vec<_>, ApplicationError>>()?;
    sources.sort_by(|left, right| left.content_hash.cmp(&right.content_hash));
    Ok(sources)
}

fn pristine_archive_bundle(collection: &PortableCollection) -> Result<&str, String> {
    validate_pristine_archive_content(collection)?;
    let Some(language_tag) = collection
        .decks
        .first()
        .and_then(|deck| deck.language_tag.as_deref())
        .filter(|language| !language.trim().is_empty())
    else {
        return Err("A bundle requires at least one deck with a language.".into());
    };
    if collection
        .decks
        .iter()
        .any(|deck| deck.language_tag.as_deref() != Some(language_tag))
    {
        return Err("Every bundle deck must use the same language.".into());
    }
    Ok(language_tag)
}

fn validate_pristine_archive_content(collection: &PortableCollection) -> Result<(), String> {
    if collection
        .notes
        .iter()
        .any(|note| note.deleted_at_ms.is_some())
    {
        return Err(
            "Bundle import is unavailable because the bundle contains trashed notes.".into(),
        );
    }
    for portable in collection.notes.iter().flat_map(|note| note.cards.iter()) {
        if !portable.review_events.is_empty() {
            return Err(
                "Bundle import is unavailable because the bundle contains review history.".into(),
            );
        }
        let schedule = &portable.schedule;
        if portable.card.suspended
            || portable.baseline != *schedule
            || schedule.version != 0
            || schedule.lifecycle != CardLifecycle::Unseen
            || schedule.interval_milliseconds != 0
            || schedule.interval_seconds != 0
            || schedule.repetitions != 0
            || schedule.stability_milliseconds != 0
            || schedule.difficulty_millipoints != 0
            || schedule.last_reviewed_at_ms.is_some()
            || schedule.last_review_event_id.is_some()
        {
            return Err(
                "Bundle import is unavailable because the bundle contains scheduled or modified cards."
                    .into(),
            );
        }
    }
    Ok(())
}

fn build_pristine_deck_import(
    collection: &PortableCollection,
    archived_deck: &meiki_domain::Deck,
    imported_at_ms: i64,
) -> PristineDeckImport {
    let mut deck = archived_deck.clone();
    deck.settings = StudySettingsOverride::default();
    deck.created_at_ms = imported_at_ms;
    deck.updated_at_ms = imported_at_ms;

    let notes = collection
        .notes
        .iter()
        .filter(|portable_note| portable_note.source_item.deck_id == archived_deck.id)
        .map(|portable_note| {
            let mut source_item = portable_note.source_item.clone();
            source_item.created_at_ms = imported_at_ms;
            source_item.updated_at_ms = imported_at_ms;
            for tag in &mut source_item.tags {
                tag.created_at_ms = imported_at_ms;
                tag.updated_at_ms = imported_at_ms;
            }
            for media in &mut source_item.media {
                media.created_at_ms = imported_at_ms;
            }
            let mut clozes = portable_note.clozes.clone();
            for cloze in &mut clozes {
                cloze.created_at_ms = imported_at_ms;
                cloze.updated_at_ms = imported_at_ms;
                for media in &mut cloze.media {
                    media.created_at_ms = imported_at_ms;
                }
            }
            let cards = portable_note
                .cards
                .iter()
                .map(|portable_card| {
                    let mut card = portable_card.card.clone();
                    card.suspended = false;
                    card.created_at_ms = imported_at_ms;
                    card.updated_at_ms = imported_at_ms;
                    let mut initial_schedule = portable_card.baseline.clone();
                    initial_schedule.version = 0;
                    initial_schedule.lifecycle = CardLifecycle::Unseen;
                    initial_schedule.due_at_ms = imported_at_ms;
                    initial_schedule.ideal_due_at_ms = imported_at_ms;
                    initial_schedule.interval_milliseconds = 0;
                    initial_schedule.interval_seconds = 0;
                    initial_schedule.repetitions = 0;
                    initial_schedule.stability_milliseconds = 0;
                    initial_schedule.difficulty_millipoints = 0;
                    initial_schedule.last_reviewed_at_ms = None;
                    initial_schedule.last_review_event_id = None;
                    PristineDeckCard {
                        card,
                        initial_schedule,
                    }
                })
                .collect();
            PristineDeckNote {
                note: StoredSourceNote {
                    source_item,
                    clozes,
                },
                cards,
            }
        })
        .collect();
    PristineDeckImport { deck, notes }
}

fn build_pristine_bundle_import(
    collection: &PortableCollection,
    imported_at_ms: i64,
) -> Result<PristineBundleImport, ApplicationError> {
    let language_tag = pristine_archive_bundle(collection)
        .map_err(ApplicationError::InvalidPortable)?
        .to_owned();
    let decks = collection
        .decks
        .iter()
        .map(|deck| build_pristine_deck_import(collection, deck, imported_at_ms))
        .collect();
    Ok(PristineBundleImport {
        language_tag,
        decks,
    })
}

fn preview_bundle_archive(
    service: &ApplicationService,
    path: &str,
    archive: &ArchivePreview,
) -> Result<BundlePreviewDto, ApplicationError> {
    let bundle = build_pristine_bundle_import(&archive.collection, 0)?;
    let plan = service
        .open_storage()?
        .validate_pristine_bundle_import(&bundle)?;
    let missing = plan
        .missing_deck_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut card_counts = HashMap::<&str, u64>::new();
    for note in &archive.collection.notes {
        let cards = count(note.cards.len(), "bundle deck card count")?;
        let current = card_counts
            .entry(note.source_item.deck_id.as_str())
            .or_default();
        *current = current
            .checked_add(cards)
            .ok_or(ApplicationError::NumericRange("bundle deck card count"))?;
    }
    let decks = archive
        .collection
        .decks
        .iter()
        .map(|deck| BundleDeckPreviewDto {
            id: deck.id.clone(),
            name: deck.name.clone(),
            cards: card_counts.get(deck.id.as_str()).copied().unwrap_or(0),
            status: if missing.contains(deck.id.as_str()) {
                BundleDeckInstallStatusDto::Missing
            } else {
                BundleDeckInstallStatusDto::Installed
            },
        })
        .collect();
    let audio_objects = bundle_media_hashes(&archive.collection, None, Some(MediaKind::Audio));
    Ok(BundlePreviewDto {
        path: path.to_owned(),
        language_tag: bundle.language_tag,
        decks,
        total_cards: archive.manifest.counts.cards,
        audio_objects: count(audio_objects.len(), "bundle audio count")?,
        can_import: plan.requires_changes(),
    })
}

fn missing_bundle_content(
    collection: &PortableCollection,
    bundle: &PristineBundleImport,
    plan: &PristineBundleImportPlan,
) -> Result<MissingBundleContent, ApplicationError> {
    let missing = plan
        .missing_deck_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut added_cards = 0_u64;
    for deck in &bundle.decks {
        if !missing.contains(deck.deck.id.as_str()) {
            continue;
        }
        let deck_cards = deck.notes.iter().try_fold(0_u64, |total, note| {
            total
                .checked_add(count(note.cards.len(), "bundle deck card count")?)
                .ok_or(ApplicationError::NumericRange("bundle card count"))
        })?;
        added_cards = added_cards
            .checked_add(deck_cards)
            .ok_or(ApplicationError::NumericRange("bundle card count"))?;
    }
    let media_hashes = bundle_media_hashes(collection, Some(&missing), None);
    let audio_hashes = bundle_media_hashes(collection, Some(&missing), Some(MediaKind::Audio));
    Ok(MissingBundleContent {
        cards: added_cards,
        media_hashes,
        audio_hashes,
    })
}

fn bundle_media_hashes(
    collection: &PortableCollection,
    deck_ids: Option<&HashSet<&str>>,
    kind: Option<MediaKind>,
) -> HashSet<String> {
    collection
        .notes
        .iter()
        .filter(|note| {
            deck_ids.is_none_or(|decks| decks.contains(note.source_item.deck_id.as_str()))
        })
        .flat_map(|note| {
            note.source_item
                .media
                .iter()
                .chain(note.clozes.iter().flat_map(|cloze| cloze.media.iter()))
        })
        .filter(|media| kind.is_none_or(|expected| media.kind == expected))
        .map(|media| media.content_hash.clone())
        .collect()
}

fn bundle_import_result(language_tag: &str) -> BundleImportResultDto {
    BundleImportResultDto {
        language_tag: language_tag.to_owned(),
        added_decks: 0,
        added_cards: 0,
        imported_media_objects: 0,
        deduplicated_media_objects: 0,
    }
}

fn count(value: usize, label: &'static str) -> Result<u64, ApplicationError> {
    u64::try_from(value).map_err(|_| ApplicationError::NumericRange(label))
}

#[derive(Default)]
struct ArchiveMediaImport {
    imported: u64,
    deduplicated: u64,
    new_hashes: Vec<String>,
}

fn import_archive_media_subset(
    archive: &ValidatedArchive,
    store: &meiki_media::MediaStore,
    included_hashes: &HashSet<String>,
    mut on_media_imported: impl FnMut(&str) -> Result<(), ApplicationError>,
) -> Result<ArchiveMediaImport, ApplicationError> {
    let mut import = ArchiveMediaImport::default();
    for media in archive
        .media_objects
        .iter()
        .filter(|media| included_hashes.contains(&media.content_hash))
    {
        let result = match store.import_file(&media.path) {
            Ok(result) => result,
            Err(error) => {
                rollback_archive_media(store, &import.new_hashes)?;
                return Err(ApplicationError::Media(error));
            }
        };
        if result.deduplicated {
            import.deduplicated += 1;
        } else {
            import.imported += 1;
            import.new_hashes.push(result.content_hash.clone());
        }
        if let Err(error) = validate_imported_archive_media(archive, media, &result) {
            rollback_archive_media(store, &import.new_hashes)?;
            return Err(error);
        }
        if let Err(error) = on_media_imported(&media.content_hash) {
            rollback_archive_media(store, &import.new_hashes)?;
            return Err(error);
        }
    }
    Ok(import)
}

fn validate_imported_archive_media(
    archive: &ValidatedArchive,
    media: &meiki_portable::ValidatedMediaObject,
    result: &meiki_media::ImportedMedia,
) -> Result<(), ApplicationError> {
    if result.content_hash != media.content_hash || result.byte_size != media.byte_size {
        return Err(ApplicationError::InvalidPortable(format!(
            "media metadata changed during import for {}",
            media.content_hash
        )));
    }
    for reference in archive
        .collection
        .notes
        .iter()
        .flat_map(|note| {
            note.source_item
                .media
                .iter()
                .chain(note.clozes.iter().flat_map(|cloze| cloze.media.iter()))
        })
        .filter(|reference| reference.content_hash == media.content_hash)
    {
        if !imported_media_metadata_matches(reference, result) {
            return Err(ApplicationError::InvalidPortable(format!(
                "media technical metadata does not match {}",
                media.content_hash
            )));
        }
    }
    Ok(())
}

fn imported_media_metadata_matches(
    reference: &MediaReference,
    result: &meiki_media::ImportedMedia,
) -> bool {
    let detected_kind = match result.kind {
        meiki_media::DetectedMediaKind::Audio => meiki_domain::MediaKind::Audio,
        meiki_media::DetectedMediaKind::Image => meiki_domain::MediaKind::Image,
    };
    reference.kind == detected_kind
        && reference.media_type == result.media_type
        && reference.byte_size == result.byte_size
        && reference.width == result.width
        && reference.height == result.height
        // The local detector currently measures WAV duration but deliberately
        // leaves compressed-audio duration unknown. A checksum-validated
        // archive may carry that optional duration without being rejected.
        && (result.duration_ms.is_none() || reference.duration_ms == result.duration_ms)
}

fn rollback_archive_media(
    store: &meiki_media::MediaStore,
    new_hashes: &[String],
) -> Result<(), ApplicationError> {
    for hash in new_hashes.iter().rev() {
        store.remove(hash)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use meiki_domain::{CardLifecycle, SchedulingMode, SegmentContent, StudySettingsOverride};
    use meiki_media::{DetectedMediaKind, ImportedMedia};
    use meiki_portable::{ArchiveMediaSource, PortableCollection, read_archive, write_archive};
    use meiki_storage::{
        DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository,
        PristineDeckRepository, SAMPLE_CARD_ID, SAMPLE_SOURCE_ID, SchedulerProfileRepository,
        SourceNoteRepository, Storage,
    };
    use tempfile::tempdir;

    use super::{
        BundleDeckInstallStatusDto, BundleExportRequest, BundleImportProgressDto,
        BundleImportRequest, BundleImportStageDto, build_pristine_bundle_import,
        imported_media_metadata_matches,
    };
    use crate::{
        ApplicationService, GradeDto, GradeReviewRequest, SchedulingModeDto, TodayRequest,
        UpdateSchedulerSettingsRequest,
    };

    const FIXTURE_SOURCE_ID: &str = "fixture-source-ja-001";
    const FIXTURE_CARD_ID: &str = "fixture-card-ja-001";
    const PRISTINE_FIXTURE: &[u8] = include_bytes!("../fixtures/pristine-deck-v4.meiki");

    fn managed_backup_count(collection_path: &Path) -> usize {
        let directory = collection_path.parent().unwrap().join("backups");
        std::fs::read_dir(directory).map_or(0, |entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|value| value == "bak"))
                .count()
        })
    }

    fn copy_pristine_fixture(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, PRISTINE_FIXTURE).unwrap();
        path
    }

    fn write_bundle_fixture(
        directory: &Path,
        name: &str,
        stage_count: usize,
        mutate: impl FnOnce(&mut PortableCollection),
    ) -> PathBuf {
        const STAGE_NAMES: [&str; 6] = [
            "Japanese 00 — Kana, sound, and Japanese input",
            "Japanese 01 — N5 / A1 foundation",
            "Japanese 02 — N4 / A2 elementary",
            "Japanese 03 — N3 / B1 intermediate",
            "Japanese 04 — N2 / B2 upper-intermediate",
            "Japanese 05 — N1 / balanced C1 bridge",
        ];

        let fixture_path = copy_pristine_fixture(directory, &format!("{name}-source.meiki"));
        let archive = read_archive(&fixture_path).unwrap();
        let deck_template = archive.collection.decks[0].clone();
        let note_template = archive.collection.notes[0].clone();
        let profile_template = archive.collection.scheduler_profiles[0].clone();
        let mut collection = archive.collection.clone();
        collection.decks.clear();
        collection.notes.clear();
        collection.scheduler_profiles.clear();
        for (stage, stage_name) in STAGE_NAMES.iter().take(stage_count).enumerate() {
            let deck_id = bundle_deck_id(stage);
            collection.decks.push(meiki_domain::Deck {
                id: deck_id.clone(),
                name: (*stage_name).into(),
                description: None,
                language_tag: Some("ja-JP".into()),
                settings: StudySettingsOverride::default(),
                ..deck_template.clone()
            });
            collection
                .scheduler_profiles
                .push(meiki_domain::SchedulerProfile {
                    deck_id: deck_id.clone(),
                    scheduling_mode: SchedulingMode::Automatic,
                    deck_daily_time_budget_minutes: None,
                    ..profile_template.clone()
                });

            let suffix = format!("-{stage:02}");
            let mut note = note_template.clone();
            note.source_item.id.push_str(&suffix);
            note.source_item.deck_id = deck_id;
            note.source_item.language_tag = Some("ja-JP".into());
            for segment in &mut note.source_item.segments {
                segment.id.push_str(&suffix);
                if let SegmentContent::Cloze { cloze_id, .. } = &mut segment.content {
                    cloze_id.push_str(&suffix);
                }
            }
            for media in &mut note.source_item.media {
                media.id.push_str(&suffix);
                media.language_tag = Some("ja-JP".into());
            }
            for cloze in &mut note.clozes {
                cloze.id.push_str(&suffix);
                cloze.source_item_id.clone_from(&note.source_item.id);
                cloze.language_tag = Some("ja-JP".into());
            }
            for card in &mut note.cards {
                card.card.id.push_str(&suffix);
                card.card.cloze_id.push_str(&suffix);
                card.baseline.card_id.clone_from(&card.card.id);
                card.schedule.card_id.clone_from(&card.card.id);
            }
            collection.notes.push(note);
        }
        mutate(&mut collection);
        let media = archive
            .media_objects
            .iter()
            .map(|object| ArchiveMediaSource {
                content_hash: object.content_hash.clone(),
                path: object.path.clone(),
            })
            .collect::<Vec<_>>();
        let destination = directory.join(format!("{name}.meiki"));
        write_archive(&destination, &collection, &media, 10_000).unwrap();
        destination
    }

    fn bundle_deck_id(stage: usize) -> String {
        format!("fixture-deck-ja-{stage:02}")
    }

    fn bundle_card_id(stage: usize) -> String {
        format!("{FIXTURE_CARD_ID}-{stage:02}")
    }

    fn bundle_source_id(stage: usize) -> String {
        format!("{FIXTURE_SOURCE_ID}-{stage:02}")
    }

    #[test]
    fn compressed_audio_can_keep_archive_duration_when_local_detection_is_unknown() {
        let reference = meiki_domain::MediaReference {
            id: "compressed-audio".into(),
            content_hash: "sha256:fixture".into(),
            kind: meiki_domain::MediaKind::Audio,
            role: meiki_domain::MediaRole::PromptAudio,
            media_type: "audio/mpeg".into(),
            byte_size: 46_125,
            original_file_name: Some("fixture.mp3".into()),
            alt_text: None,
            width: None,
            height: None,
            duration_ms: Some(2_800),
            language_tag: Some("ja-JP".into()),
            direction: meiki_domain::Direction::Auto,
            created_at_ms: 0,
        };
        let mut detected = ImportedMedia {
            content_hash: reference.content_hash.clone(),
            kind: DetectedMediaKind::Audio,
            media_type: reference.media_type.clone(),
            byte_size: reference.byte_size,
            original_file_name: "fixture.mp3".into(),
            width: None,
            height: None,
            duration_ms: None,
            object_path: PathBuf::from("fixture"),
            deduplicated: false,
        };
        assert!(imported_media_metadata_matches(&reference, &detected));
        detected.duration_ms = Some(2_799);
        assert!(!imported_media_metadata_matches(&reference, &detected));
    }

    #[test]
    fn first_bundle_import_adds_six_independent_study_decks() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        service.seed_test_collection(100_000).unwrap();
        let archive_path = write_bundle_fixture(directory.path(), "bundle-first", 6, |_| {});

        let result = service
            .import_bundle(
                &BundleImportRequest {
                    path: archive_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap();

        assert_eq!((result.added_decks, result.added_cards), (6, 6));
        let storage = Storage::open(&collection_path).unwrap();
        assert_eq!(storage.list_decks().unwrap().len(), 7);
        assert!(storage.get_source_note(SAMPLE_SOURCE_ID).is_ok());
        drop(storage);
        for stage in 0..6 {
            let card = service.get_study_card(&bundle_card_id(stage)).unwrap();
            assert_eq!(card.card_id, bundle_card_id(stage));
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn bundle_import_adds_missing_stages_and_preserves_existing_learning_state() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let original_card = service.seed_test_collection(100_000).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-before-bundle-add".into(),
                    card_id: original_card.card_id,
                    card_content_version: original_card.card_content_version,
                    schedule_version: original_card.schedule_version,
                    raw_response: "行きます".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 1_000,
                },
                100_000,
            )
            .unwrap();
        service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: DEFAULT_DECK_ID.into(),
                scheduling_mode: SchedulingModeDto::Automatic,
                collection_daily_time_budget_minutes: 1,
                deck_daily_time_budget_minutes: None,
                target_retention_basis_points: 9_000,
                new_cards_per_day: 20,
                maximum_interval_days: 36_500,
                day_boundary_minutes: 240,
                now_ms: 110_000,
                day_start_ms: 0,
            })
            .unwrap();
        let (original_schedule, original_history, original_settings) = {
            let storage = Storage::open(&collection_path).unwrap();
            (
                storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
                storage.review_events(SAMPLE_CARD_ID).unwrap(),
                storage.collection_scheduling_settings().unwrap(),
            )
        };

        let partial_path = write_bundle_fixture(directory.path(), "bundle-partial", 2, |_| {});
        let full_path = write_bundle_fixture(directory.path(), "bundle-full", 6, |_| {});
        let initial_preview = service
            .preview_bundle(&full_path.to_string_lossy())
            .unwrap();
        assert_eq!(initial_preview.language_tag, "ja-JP");
        assert_eq!(initial_preview.decks.len(), 6);
        assert_eq!(initial_preview.total_cards, 6);
        assert_eq!(initial_preview.audio_objects, 1);
        assert!(initial_preview.can_import);
        assert!(
            initial_preview
                .decks
                .iter()
                .all(|deck| deck.status == BundleDeckInstallStatusDto::Missing)
        );

        let partial = service
            .import_bundle(
                &BundleImportRequest {
                    path: partial_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap();
        assert_eq!((partial.added_decks, partial.added_cards), (2, 2));
        assert_eq!(
            (
                partial.imported_media_objects,
                partial.deduplicated_media_objects
            ),
            (1, 0)
        );

        let imported_study = service.get_study_card(&bundle_card_id(0)).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-imported-stage".into(),
                    card_id: imported_study.card_id,
                    card_content_version: imported_study.card_content_version,
                    schedule_version: imported_study.schedule_version,
                    raw_response: "晴れです".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 800,
                },
                410_000,
            )
            .unwrap();
        let (reviewed_stage_schedule, reviewed_stage_history) = {
            let storage = Storage::open(&collection_path).unwrap();
            (
                storage.load_schedule(&bundle_card_id(0)).unwrap(),
                storage.review_events(&bundle_card_id(0)).unwrap(),
            )
        };

        let partial_preview = service
            .preview_bundle(&full_path.to_string_lossy())
            .unwrap();
        assert_eq!(
            partial_preview
                .decks
                .iter()
                .map(|deck| deck.status)
                .collect::<Vec<_>>(),
            [
                BundleDeckInstallStatusDto::Installed,
                BundleDeckInstallStatusDto::Installed,
                BundleDeckInstallStatusDto::Missing,
                BundleDeckInstallStatusDto::Missing,
                BundleDeckInstallStatusDto::Missing,
                BundleDeckInstallStatusDto::Missing,
            ]
        );

        let mut progress = Vec::new();
        let completed = service
            .import_bundle(
                &BundleImportRequest {
                    path: full_path.to_string_lossy().into_owned(),
                    now_ms: 500_000,
                },
                |update| progress.push(update),
            )
            .unwrap();
        assert_eq!((completed.added_decks, completed.added_cards), (4, 4));
        assert_eq!(
            (
                completed.imported_media_objects,
                completed.deduplicated_media_objects
            ),
            (0, 1)
        );
        assert_eq!(
            progress,
            [
                BundleImportProgressDto {
                    stage: BundleImportStageDto::PreparingDecks,
                    current: 0,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::PreparingDecks,
                    current: 4,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingCards,
                    current: 0,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingCards,
                    current: 1,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingCards,
                    current: 2,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingCards,
                    current: 3,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingCards,
                    current: 4,
                    total: 4,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingAudio,
                    current: 0,
                    total: 1,
                },
                BundleImportProgressDto {
                    stage: BundleImportStageDto::AddingAudio,
                    current: 1,
                    total: 1,
                },
            ]
        );

        let storage = Storage::open(&collection_path).unwrap();
        assert_eq!(
            storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
            original_schedule
        );
        assert_eq!(
            storage.review_events(SAMPLE_CARD_ID).unwrap(),
            original_history
        );
        assert_eq!(
            storage.collection_scheduling_settings().unwrap(),
            original_settings
        );
        assert_eq!(
            storage.load_schedule(&bundle_card_id(0)).unwrap(),
            reviewed_stage_schedule
        );
        assert_eq!(
            storage.review_events(&bundle_card_id(0)).unwrap(),
            reviewed_stage_history
        );
        for stage in 0..6 {
            let profile = storage
                .get_scheduler_profile(&bundle_deck_id(stage))
                .unwrap();
            assert_eq!(profile.scheduling_mode, SchedulingMode::Automatic);
            assert_eq!(profile.deck_daily_time_budget_minutes, None);
        }
        drop(storage);

        let today = service
            .get_today_overview(&TodayRequest {
                deck_id: bundle_deck_id(5),
                now_ms: 500_000,
                day_start_ms: 0,
                day_end_ms: 86_400_000,
            })
            .unwrap();
        assert_eq!(today.new_cards + today.deferred_new_cards, 1);
        assert_eq!(
            service
                .get_study_card(&bundle_card_id(5))
                .unwrap()
                .prompt_media
                .len(),
            1
        );

        let installed_preview = service
            .preview_bundle(&full_path.to_string_lossy())
            .unwrap();
        assert!(!installed_preview.can_import);
        assert!(
            installed_preview
                .decks
                .iter()
                .all(|deck| deck.status == BundleDeckInstallStatusDto::Installed)
        );
        let backups_before_no_op = managed_backup_count(&collection_path);
        let mut no_op_progress = Vec::new();
        let no_op = service
            .import_bundle(
                &BundleImportRequest {
                    path: full_path.to_string_lossy().into_owned(),
                    now_ms: 600_000,
                },
                |update| no_op_progress.push(update),
            )
            .unwrap();
        assert_eq!((no_op.added_decks, no_op.added_cards), (0, 0));
        assert!(no_op_progress.is_empty());
        assert_eq!(managed_backup_count(&collection_path), backups_before_no_op);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn bundle_import_associates_existing_stages_without_changing_their_learning_state() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let archive_path = write_bundle_fixture(directory.path(), "bundle-existing", 6, |_| {});
        let archive = read_archive(&archive_path).unwrap();
        let bundle = build_pristine_bundle_import(&archive.collection, 400_000).unwrap();
        let mut storage = Storage::open(&collection_path).unwrap();
        for stage in &bundle.decks {
            assert_eq!(
                storage.import_pristine_deck(stage).unwrap(),
                meiki_storage::PristineDeckImportStatus::Ready
            );
        }
        let first_deck_id = bundle_deck_id(0);
        let mut personalized_deck = storage.get_deck(&first_deck_id).unwrap();
        personalized_deck.name = "My Japanese foundation".into();
        personalized_deck.settings = StudySettingsOverride {
            new_cards_per_day: Some(7),
            ..StudySettingsOverride::default()
        };
        personalized_deck.updated_at_ms = 410_000;
        storage.update_deck(&personalized_deck).unwrap();
        drop(storage);

        service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: first_deck_id.clone(),
                scheduling_mode: SchedulingModeDto::Expert,
                collection_daily_time_budget_minutes: 25,
                deck_daily_time_budget_minutes: Some(12),
                target_retention_basis_points: 9_200,
                new_cards_per_day: 7,
                maximum_interval_days: 20_000,
                day_boundary_minutes: 240,
                now_ms: 420_000,
                day_start_ms: 0,
            })
            .unwrap();
        let card = service.get_study_card(&bundle_card_id(0)).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-before-association".into(),
                    card_id: card.card_id,
                    card_content_version: card.card_content_version,
                    schedule_version: card.schedule_version,
                    raw_response: "晴れです".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 700,
                },
                430_000,
            )
            .unwrap();
        let before = {
            let storage = Storage::open(&collection_path).unwrap();
            (
                storage.get_deck(&first_deck_id).unwrap(),
                storage.load_study_card(&bundle_card_id(0)).unwrap(),
                storage.review_events(&bundle_card_id(0)).unwrap(),
                storage.get_scheduler_profile(&first_deck_id).unwrap(),
                storage.collection_scheduling_settings().unwrap(),
            )
        };

        let preview = service
            .preview_bundle(&archive_path.to_string_lossy())
            .unwrap();
        assert!(preview.can_import);
        assert!(
            preview
                .decks
                .iter()
                .all(|deck| deck.status == BundleDeckInstallStatusDto::Installed)
        );
        let result = service
            .import_bundle(
                &BundleImportRequest {
                    path: archive_path.to_string_lossy().into_owned(),
                    now_ms: 500_000,
                },
                |_| {},
            )
            .unwrap();
        assert_eq!((result.added_decks, result.added_cards), (0, 0));
        assert_eq!(
            (
                result.imported_media_objects,
                result.deduplicated_media_objects
            ),
            (0, 0)
        );

        let storage = Storage::open(&collection_path).unwrap();
        let after = (
            storage.get_deck(&first_deck_id).unwrap(),
            storage.load_study_card(&bundle_card_id(0)).unwrap(),
            storage.review_events(&bundle_card_id(0)).unwrap(),
            storage.get_scheduler_profile(&first_deck_id).unwrap(),
            storage.collection_scheduling_settings().unwrap(),
        );
        assert_eq!(after, before);
        let installed = storage.validate_pristine_bundle_import(&bundle).unwrap();
        assert_eq!(installed.installed_deck_ids.len(), 6);
        assert!(installed.missing_deck_ids.is_empty());
        assert!(installed.unassociated_deck_ids.is_empty());
        drop(storage);
        assert!(
            !service
                .preview_bundle(&archive_path.to_string_lossy())
                .unwrap()
                .can_import
        );
    }

    #[test]
    fn bundle_import_failure_after_processing_media_leaves_no_content_or_media() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let archive_path =
            write_bundle_fixture(directory.path(), "bundle-invalid-media", 2, |collection| {
                for media in collection
                    .notes
                    .iter_mut()
                    .flat_map(|note| note.source_item.media.iter_mut())
                {
                    media.media_type = "audio/mpeg".into();
                }
            });
        assert!(
            service
                .preview_bundle(&archive_path.to_string_lossy())
                .unwrap()
                .can_import
        );

        let error = service
            .import_bundle(
                &BundleImportRequest {
                    path: archive_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap_err();

        assert!(error.to_string().contains("media technical metadata"));
        let storage = Storage::open(&collection_path).unwrap();
        assert_eq!(storage.list_decks().unwrap().len(), 1);
        assert!(storage.get_deck(&bundle_deck_id(0)).is_err());
        assert!(storage.load_study_card(&bundle_card_id(0)).is_err());
        drop(storage);
        assert!(
            service
                .media_store()
                .resolve("sha256:4f8734c5e13ac599e168cf247a51c1dd0758537ce00bf16d7fed1a3d14d07041")
                .is_err()
        );
    }

    #[test]
    fn bundle_transaction_fault_after_one_media_object_rolls_back_database_and_media() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let archive_path = write_bundle_fixture(directory.path(), "bundle-media-fault", 2, |_| {});
        let archive = read_archive(&archive_path).unwrap();
        let bundle = build_pristine_bundle_import(&archive.collection, 400_000).unwrap();
        let mut storage = Storage::open(&collection_path).unwrap();
        let plan = storage.validate_pristine_bundle_import(&bundle).unwrap();
        let missing = super::missing_bundle_content(&archive.collection, &bundle, &plan).unwrap();
        let media_store = service.media_store();
        let media_hash = archive.media_objects[0].content_hash.clone();
        let mut processed_media = 0;

        let result = storage.import_pristine_bundle(
            &bundle,
            || {},
            || {
                super::import_archive_media_subset(
                    &archive,
                    &media_store,
                    &missing.media_hashes,
                    |_| {
                        processed_media += 1;
                        Err(crate::ApplicationError::InvalidPortable(
                            "injected fault after media processing".into(),
                        ))
                    },
                )
            },
        );

        assert_eq!(processed_media, 1);
        assert!(matches!(
            result,
            Err(meiki_storage::PristineBundleImportError::BeforeCommit(
                crate::ApplicationError::InvalidPortable(message)
            )) if message == "injected fault after media processing"
        ));
        drop(storage);
        let reopened = Storage::open(&collection_path).unwrap();
        assert!(reopened.get_deck(&bundle_deck_id(0)).is_err());
        assert!(reopened.load_study_card(&bundle_card_id(0)).is_err());
        drop(reopened);
        assert!(media_store.resolve(&media_hash).is_err());
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn bundle_export_contains_only_pristine_active_content_and_imports_additively() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        service.seed_test_collection(100_000).unwrap();
        let bundle_path = write_bundle_fixture(directory.path(), "bundle-export", 5, |_| {});
        service
            .import_bundle(
                &BundleImportRequest {
                    path: bundle_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap();

        let reviewed = service.get_study_card(&bundle_card_id(0)).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "bundle-export-review".into(),
                    card_id: reviewed.card_id,
                    card_content_version: reviewed.card_content_version,
                    schedule_version: reviewed.schedule_version,
                    raw_response: "晴れです".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 800,
                },
                410_000,
            )
            .unwrap();

        let mut storage = Storage::open(&collection_path).unwrap();
        storage
            .move_deck_cards(&[SAMPLE_CARD_ID.into()], &bundle_deck_id(0), 420_000)
            .unwrap();
        storage
            .set_deck_cards_suspended(&[SAMPLE_CARD_ID.into()], true, 421_000)
            .unwrap();
        storage
            .move_deck_cards(&[bundle_card_id(2)], DEFAULT_DECK_ID, 430_000)
            .unwrap();
        storage
            .set_deck_cards_deleted(&[bundle_card_id(3)], Some(440_000), 440_000)
            .unwrap();
        storage
            .delete_deck_and_rehome_notes(&bundle_deck_id(4), None, 450_000)
            .unwrap();
        drop(storage);

        service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: bundle_deck_id(0),
                scheduling_mode: SchedulingModeDto::Expert,
                collection_daily_time_budget_minutes: 777,
                deck_daily_time_budget_minutes: Some(321),
                target_retention_basis_points: 9_500,
                new_cards_per_day: 999,
                maximum_interval_days: 12_345,
                day_boundary_minutes: 600,
                now_ms: 460_000,
                day_start_ms: 0,
            })
            .unwrap();
        let stable_stage = Storage::open(&collection_path)
            .unwrap()
            .get_source_note(&bundle_source_id(0))
            .unwrap();
        let stable_cloze_ids = stable_stage
            .clozes
            .iter()
            .map(|cloze| cloze.id.clone())
            .collect::<Vec<_>>();
        let stable_media_reference_ids = stable_stage
            .source_item
            .media
            .iter()
            .chain(
                stable_stage
                    .clozes
                    .iter()
                    .flat_map(|cloze| cloze.media.iter()),
            )
            .map(|media| media.id.clone())
            .collect::<Vec<_>>();

        let exported = service
            .export_bundle(&BundleExportRequest {
                language_tag: "ja-JP".into(),
                now_ms: 500_000,
            })
            .unwrap();
        assert_eq!(
            (
                exported.decks,
                exported.notes,
                exported.cards,
                exported.review_events,
                exported.media_objects,
            ),
            (4, 3, 3, 0, 1)
        );

        let archive = read_archive(Path::new(&exported.path)).unwrap();
        assert_eq!(
            archive
                .collection
                .decks
                .iter()
                .map(|deck| deck.id.as_str())
                .collect::<Vec<_>>(),
            [
                bundle_deck_id(0),
                bundle_deck_id(1),
                bundle_deck_id(2),
                bundle_deck_id(3),
            ]
        );
        assert!(
            archive
                .collection
                .decks
                .iter()
                .all(|deck| deck.settings == StudySettingsOverride::default())
        );
        assert_eq!(
            archive.collection.collection_scheduling_settings,
            meiki_domain::CollectionSchedulingSettings::default()
        );
        assert_eq!(archive.collection.scheduler_parameter_sets.len(), 1);
        assert_eq!(
            archive.collection.scheduler_parameter_sets[0].id,
            DEFAULT_SCHEDULER_PARAMETER_SET_ID
        );
        assert!(archive.collection.scheduler_profiles.iter().all(|profile| {
            profile.scheduling_mode == SchedulingMode::Automatic
                && profile.active_parameter_set_id == DEFAULT_SCHEDULER_PARAMETER_SET_ID
                && profile.deck_daily_time_budget_minutes.is_none()
                && profile.controller_last_evaluated_day_start_ms.is_none()
                && profile.controller_review_count == 0
                && profile.controller_unseen_count == 0
                && profile.controller_forecast_review_seconds_per_day == 0
                && !profile.controller_backlog_exceeds_budget
                && profile.controller_explanation.is_empty()
        }));

        let exported_source_ids = archive
            .collection
            .notes
            .iter()
            .map(|note| note.source_item.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            exported_source_ids,
            [
                bundle_source_id(0),
                bundle_source_id(1),
                SAMPLE_SOURCE_ID.into(),
            ]
        );
        assert!(!exported_source_ids.contains(&bundle_source_id(2).as_str()));
        assert!(!exported_source_ids.contains(&bundle_source_id(3).as_str()));
        let exported_stage = archive
            .collection
            .notes
            .iter()
            .find(|note| note.source_item.id == stable_stage.source_item.id)
            .unwrap();
        assert_eq!(
            exported_stage
                .clozes
                .iter()
                .map(|cloze| &cloze.id)
                .collect::<Vec<_>>(),
            stable_cloze_ids.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            exported_stage
                .source_item
                .media
                .iter()
                .chain(
                    exported_stage
                        .clozes
                        .iter()
                        .flat_map(|cloze| cloze.media.iter()),
                )
                .map(|media| &media.id)
                .collect::<Vec<_>>(),
            stable_media_reference_ids.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            exported_stage.source_item.annotations,
            stable_stage.source_item.annotations
        );
        assert_eq!(
            exported_stage
                .clozes
                .iter()
                .flat_map(|cloze| cloze.annotations.iter())
                .collect::<Vec<_>>(),
            stable_stage
                .clozes
                .iter()
                .flat_map(|cloze| cloze.annotations.iter())
                .collect::<Vec<_>>()
        );
        for portable in archive
            .collection
            .notes
            .iter()
            .flat_map(|note| note.cards.iter())
        {
            assert!(!portable.card.suspended);
            assert_eq!(portable.baseline, portable.schedule);
            assert_eq!(portable.schedule.version, 0);
            assert_eq!(portable.schedule.lifecycle, CardLifecycle::Unseen);
            assert_eq!(portable.schedule.due_at_ms, 0);
            assert_eq!(portable.schedule.ideal_due_at_ms, 0);
            assert!(portable.review_events.is_empty());
        }

        let imported_path = directory.path().join("imported.db");
        let imported_service = ApplicationService::new(&imported_path);
        let imported = imported_service
            .import_bundle(
                &BundleImportRequest {
                    path: exported.path,
                    now_ms: 600_000,
                },
                |_| {},
            )
            .unwrap();
        assert_eq!(
            (
                imported.added_decks,
                imported.added_cards,
                imported.imported_media_objects,
            ),
            (4, 3, 1)
        );
        let imported_storage = Storage::open(&imported_path).unwrap();
        let imported_user_note = imported_storage.get_source_note(SAMPLE_SOURCE_ID).unwrap();
        assert_eq!(imported_user_note.source_item.deck_id, bundle_deck_id(0));
        let imported_stage = imported_storage
            .get_source_note(&stable_stage.source_item.id)
            .unwrap();
        assert_eq!(
            imported_stage
                .clozes
                .iter()
                .map(|cloze| &cloze.id)
                .collect::<Vec<_>>(),
            stable_cloze_ids.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            imported_stage
                .source_item
                .media
                .iter()
                .chain(
                    imported_stage
                        .clozes
                        .iter()
                        .flat_map(|cloze| cloze.media.iter()),
                )
                .map(|media| &media.id)
                .collect::<Vec<_>>(),
            stable_media_reference_ids.iter().collect::<Vec<_>>()
        );
        for (card_id, deck_id) in [
            (bundle_card_id(0), bundle_deck_id(0)),
            (bundle_card_id(1), bundle_deck_id(1)),
            (SAMPLE_CARD_ID.into(), bundle_deck_id(0)),
        ] {
            let card = imported_storage.load_study_card(&card_id).unwrap();
            assert_eq!(card.source_item.deck_id, deck_id);
            assert_eq!(card.schedule.version, 0);
            assert_eq!(card.schedule.lifecycle, CardLifecycle::Unseen);
            assert_eq!(card.schedule.due_at_ms, 600_000);
            assert!(imported_storage.review_events(&card_id).unwrap().is_empty());
        }
        drop(imported_storage);
        for (deck_id, expected_new_cards) in [(bundle_deck_id(0), 2), (bundle_deck_id(1), 1)] {
            let overview = imported_service
                .get_today_overview(&TodayRequest {
                    deck_id,
                    now_ms: 600_000,
                    day_start_ms: 0,
                    day_end_ms: 86_400_000,
                })
                .unwrap();
            assert_eq!(
                overview.new_cards + overview.deferred_new_cards,
                expected_new_cards
            );
        }
    }

    #[test]
    fn canonical_bundle_import_restores_a_missing_middle_stage_after_partial_export() {
        let directory = tempdir().unwrap();
        let source_path = directory.path().join("source.db");
        let source = ApplicationService::new(&source_path);
        let canonical_path =
            write_bundle_fixture(directory.path(), "canonical-after-partial", 6, |_| {});
        source
            .import_bundle(
                &BundleImportRequest {
                    path: canonical_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap();
        Storage::open(&source_path)
            .unwrap()
            .delete_deck_and_rehome_notes(&bundle_deck_id(2), None, 410_000)
            .unwrap();
        let partial = source
            .export_bundle(&BundleExportRequest {
                language_tag: "ja-JP".into(),
                now_ms: 420_000,
            })
            .unwrap();

        let target_path = directory.path().join("target.db");
        let target = ApplicationService::new(&target_path);
        let partial_import = target
            .import_bundle(
                &BundleImportRequest {
                    path: partial.path,
                    now_ms: 500_000,
                },
                |_| {},
            )
            .unwrap();
        assert_eq!(
            (partial_import.added_decks, partial_import.added_cards),
            (5, 5)
        );
        let reviewed = target.get_study_card(&bundle_card_id(3)).unwrap();
        target
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-before-canonical-completion".into(),
                    card_id: reviewed.card_id,
                    card_content_version: reviewed.card_content_version,
                    schedule_version: reviewed.schedule_version,
                    raw_response: "晴れです".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 700,
                },
                510_000,
            )
            .unwrap();
        let (schedule_before, history_before) = {
            let storage = Storage::open(&target_path).unwrap();
            (
                storage.load_schedule(&bundle_card_id(3)).unwrap(),
                storage.review_events(&bundle_card_id(3)).unwrap(),
            )
        };

        let completed = target
            .import_bundle(
                &BundleImportRequest {
                    path: canonical_path.to_string_lossy().into_owned(),
                    now_ms: 600_000,
                },
                |_| {},
            )
            .unwrap();
        assert_eq!((completed.added_decks, completed.added_cards), (1, 1));
        let storage = Storage::open(&target_path).unwrap();
        assert_eq!(
            storage.bundle_deck_ids("ja-JP").unwrap(),
            (0..6).map(bundle_deck_id).collect::<Vec<_>>()
        );
        assert_eq!(
            storage.load_schedule(&bundle_card_id(3)).unwrap(),
            schedule_before
        );
        assert_eq!(
            storage.review_events(&bundle_card_id(3)).unwrap(),
            history_before
        );
    }

    #[test]
    fn bundle_export_failure_leaves_the_collection_and_final_files_unchanged() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let bundle_path =
            write_bundle_fixture(directory.path(), "bundle-export-failure", 1, |_| {});
        service
            .import_bundle(
                &BundleImportRequest {
                    path: bundle_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap();
        let storage = Storage::open(&collection_path).unwrap();
        let before_decks = storage.list_decks().unwrap();
        let before_notes = storage.library_notes().unwrap();
        let before_associations = storage.bundle_deck_ids("ja-JP").unwrap();
        let media_hash = storage
            .get_source_note(&bundle_source_id(0))
            .unwrap()
            .source_item
            .media[0]
            .content_hash
            .clone();
        drop(storage);
        std::fs::write(
            service.media_store().resolve(&media_hash).unwrap(),
            b"corrupt media",
        )
        .unwrap();

        assert!(
            service
                .export_bundle(&BundleExportRequest {
                    language_tag: "ja-JP".into(),
                    now_ms: 500_000,
                })
                .is_err()
        );
        let exports = directory.path().join("exports");
        assert!(!exports.exists() || std::fs::read_dir(exports).unwrap().next().is_none());
        let storage = Storage::open(&collection_path).unwrap();
        assert_eq!(storage.list_decks().unwrap(), before_decks);
        assert_eq!(storage.library_notes().unwrap(), before_notes);
        assert_eq!(
            storage.bundle_deck_ids("ja-JP").unwrap(),
            before_associations
        );
    }

    #[test]
    fn bundle_export_reports_when_no_associated_deck_remains() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let bundle_path = write_bundle_fixture(directory.path(), "bundle-export-empty", 1, |_| {});
        service
            .import_bundle(
                &BundleImportRequest {
                    path: bundle_path.to_string_lossy().into_owned(),
                    now_ms: 400_000,
                },
                |_| {},
            )
            .unwrap();
        Storage::open(&collection_path)
            .unwrap()
            .delete_deck_and_rehome_notes(&bundle_deck_id(0), None, 500_000)
            .unwrap();

        let error = service
            .export_bundle(&BundleExportRequest {
                language_tag: "ja-JP".into(),
                now_ms: 600_000,
            })
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("No installed decks remain for ja-JP.")
        );
    }
}
