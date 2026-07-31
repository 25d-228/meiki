use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{ApplicationError, ApplicationService};
use meiki_domain::{CardLifecycle, MediaReference, StudySettingsOverride};
use meiki_portable::{
    ArchiveMediaSource, ArchiveScope, PortableCard, PortableCollection, PortableNote,
    ValidatedArchive, read_archive, write_archive,
};
use meiki_storage::{
    CardRepository, DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository,
    PristineDeckCard, PristineDeckImport, PristineDeckImportStatus, PristineDeckNote,
    PristineDeckRepository, SchedulerParameterSetRepository, SchedulerProfileRepository,
    SourceNoteRepository, Storage, StorageError, StoredSourceNote,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

const BACKUP_RETENTION: usize = 5;
const REPLACE_CONFIRMATION: &str = "REPLACE";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveExportRequest {
    #[ts(type = "number")]
    pub now_ms: i64,
}

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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveImportRequest {
    pub path: String,
    pub confirmation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveAddDeckRequest {
    pub path: String,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct PortableArchivePreviewDto {
    pub path: String,
    pub format_version: u32,
    pub deck_name: Option<String>,
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
    #[ts(type = "number")]
    pub duplicate_media_objects: u64,
    pub can_add_deck: bool,
    pub add_deck_summary: String,
    pub can_import: bool,
    pub confirmation: String,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveAddDeckResultDto {
    pub backup_path: String,
    pub deck_id: String,
    pub deck_name: String,
    #[ts(type = "number")]
    pub imported_notes: u64,
    #[ts(type = "number")]
    pub imported_cards: u64,
    #[ts(type = "number")]
    pub imported_media_objects: u64,
    #[ts(type = "number")]
    pub deduplicated_media_objects: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveImportResultDto {
    pub backup_path: String,
    #[ts(type = "number")]
    pub imported_notes: u64,
    #[ts(type = "number")]
    pub imported_cards: u64,
    #[ts(type = "number")]
    pub imported_media_objects: u64,
    #[ts(type = "number")]
    pub deduplicated_media_objects: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct BackupDto {
    pub path: String,
    pub file_name: String,
    #[ts(type = "number")]
    pub byte_size: u64,
}

impl ApplicationService {
    /// Exports the complete collection as a versioned `.meiki` archive.
    ///
    /// # Errors
    ///
    /// Returns an error when stored aggregates are inconsistent, media is
    /// unavailable, the timestamp is invalid, or writing fails.
    pub fn export_archive(
        &self,
        request: &ArchiveExportRequest,
    ) -> Result<PortableExportResultDto, ApplicationError> {
        validate_export_request(request)?;
        let storage = self.open_storage()?;
        let collection = build_collection(&storage)?;
        let media = media_sources(&collection, &self.media_store())?;
        let directory = self.export_directory()?;
        let path = directory.join(format!(
            "meiki-{}-{}.meiki",
            request.now_ms,
            self.next_id("portable-archive")
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

    /// Validates a complete archive and reports what replacement would do
    /// without changing the collection.
    ///
    /// # Errors
    ///
    /// Returns an error when the archive or the current media store is invalid.
    pub fn preview_archive(
        &self,
        path: &str,
    ) -> Result<PortableArchivePreviewDto, ApplicationError> {
        let archive = read_archive(Path::new(path))?;
        preview_validated_archive(self, path, &archive)
    }

    /// Adds one validated pristine archive deck without replacing the current
    /// collection.
    ///
    /// The current collection receives a recovery backup before media or
    /// database state changes. Imported content is timestamped at the request
    /// time, uses the current default scheduler parameters, and is committed in
    /// one database transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the request time is invalid, the archive is not a
    /// pristine single-deck archive, an identity collides, media staging fails,
    /// or the database transaction cannot be committed.
    pub fn add_archive_deck(
        &self,
        request: &ArchiveAddDeckRequest,
    ) -> Result<ArchiveAddDeckResultDto, ApplicationError> {
        if request.now_ms < 0 {
            return Err(ApplicationError::InvalidPortable(
                "archive deck import timestamp is invalid".into(),
            ));
        }

        let archive = read_archive(Path::new(&request.path))?;
        let deck = pristine_archive_deck(&archive.collection)
            .map_err(ApplicationError::InvalidPortable)?;
        let deck_id = deck.id.clone();
        let deck_name = deck.name.clone();
        let pristine_import = build_pristine_deck_import(&archive.collection, request.now_ms);
        let mut storage = self.open_storage()?;
        match storage.validate_pristine_deck_import(&pristine_import)? {
            PristineDeckImportStatus::Ready => {}
            PristineDeckImportStatus::AlreadyInstalled => {
                return Err(ApplicationError::InvalidPortable(
                    "Deck already installed".into(),
                ));
            }
        }

        let backup_path = self.create_recovery_backup(&storage, "pre-add-deck")?;
        let media_import = import_archive_media(&archive, &self.media_store())?;
        let import_status = storage.import_pristine_deck(&pristine_import);
        match import_status {
            Ok(PristineDeckImportStatus::Ready) => {}
            Ok(PristineDeckImportStatus::AlreadyInstalled) => {
                rollback_archive_media(&self.media_store(), &media_import.new_hashes)?;
                return Err(ApplicationError::InvalidPortable(
                    "Deck already installed".into(),
                ));
            }
            Err(error) => {
                rollback_archive_media(&self.media_store(), &media_import.new_hashes)?;
                return Err(ApplicationError::Storage(error));
            }
        }

        Ok(ArchiveAddDeckResultDto {
            backup_path: backup_path.to_string_lossy().into_owned(),
            deck_id,
            deck_name,
            imported_notes: archive.manifest.counts.notes,
            imported_cards: archive.manifest.counts.cards,
            imported_media_objects: media_import.imported,
            deduplicated_media_objects: media_import.deduplicated,
        })
    }

    /// Imports a previously previewable archive through a staging database.
    ///
    /// The current collection is backed up immediately before its database is
    /// atomically replaced. A failed staging operation leaves the live
    /// collection unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid confirmation, a non-full archive, archive
    /// validation, staging, media, backup, or restore.
    pub fn import_archive(
        &self,
        request: &ArchiveImportRequest,
    ) -> Result<ArchiveImportResultDto, ApplicationError> {
        if request.confirmation != REPLACE_CONFIRMATION {
            return Err(ApplicationError::InvalidPortable(format!(
                "type {REPLACE_CONFIRMATION} to confirm this replacement"
            )));
        }

        let archive = read_archive(Path::new(&request.path))?;
        let preview = preview_validated_archive(self, &request.path, &archive)?;
        if !preview.can_import {
            return Err(ApplicationError::InvalidPortable(preview.summary));
        }

        let temporary = tempfile::tempdir().map_err(ApplicationError::PortableIo)?;
        let staging_path = temporary.path().join("collection.db");
        let current = self.open_storage()?;
        let mut staging = Storage::open(&staging_path)?;
        populate_staging(&mut staging, &archive.collection)?;

        let backup_path = self.create_recovery_backup(&current, "pre-import")?;
        let media_import = import_archive_media(&archive, &self.media_store())?;
        drop(staging);
        drop(current);
        if let Err(error) = Storage::replace_from_backup(&staging_path, &self.collection_path) {
            rollback_archive_media(&self.media_store(), &media_import.new_hashes)?;
            return Err(ApplicationError::Storage(error));
        }

        Ok(ArchiveImportResultDto {
            backup_path: backup_path.to_string_lossy().into_owned(),
            imported_notes: archive.manifest.counts.notes,
            imported_cards: archive.manifest.counts.cards,
            imported_media_objects: media_import.imported,
            deduplicated_media_objects: media_import.deduplicated,
        })
    }

    /// Lists managed rolling database backups from newest to oldest.
    ///
    /// # Errors
    ///
    /// Returns an error when the backup directory cannot be read safely.
    pub fn list_backups(&self) -> Result<Vec<BackupDto>, ApplicationError> {
        let directory = self.backup_directory();
        if !directory.exists() {
            return Ok(Vec::new());
        }
        let mut backups = fs::read_dir(&directory)
            .map_err(ApplicationError::PortableIo)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(ApplicationError::PortableIo)?
            .into_iter()
            .filter_map(|entry| {
                let path = entry.path();
                let file_name = entry.file_name().to_str()?.to_owned();
                (path.is_file()
                    && Path::new(&file_name)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("bak")))
                .then_some((path, file_name))
            })
            .map(|(path, file_name)| {
                let byte_size = fs::metadata(&path)
                    .map_err(ApplicationError::PortableIo)?
                    .len();
                let modified = fs::metadata(&path)
                    .and_then(|metadata| metadata.modified())
                    .map_err(ApplicationError::PortableIo)?;
                Ok((
                    BackupDto {
                        path: path.to_string_lossy().into_owned(),
                        file_name,
                        byte_size,
                    },
                    modified,
                ))
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?;
        backups.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
        Ok(backups.into_iter().map(|(backup, _)| backup).collect())
    }

    /// Restores one managed rolling backup after exact filename confirmation.
    ///
    /// A new pre-restore backup is created before replacement.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is outside the managed directory,
    /// confirmation does not match, or backup/restore validation fails.
    pub fn restore_backup(
        &self,
        path: &str,
        confirmation: &str,
    ) -> Result<BackupDto, ApplicationError> {
        let requested = fs::canonicalize(path).map_err(ApplicationError::PortableIo)?;
        let directory =
            fs::canonicalize(self.backup_directory()).map_err(ApplicationError::PortableIo)?;
        if requested.parent() != Some(directory.as_path()) {
            return Err(ApplicationError::InvalidPortable(
                "only a managed rolling backup can be restored".into(),
            ));
        }
        let file_name = requested
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                ApplicationError::InvalidPortable("backup filename is invalid".into())
            })?;
        if confirmation != file_name {
            return Err(ApplicationError::InvalidPortable(
                "type the exact backup filename to confirm restore".into(),
            ));
        }
        let temporary = tempfile::tempdir().map_err(ApplicationError::PortableIo)?;
        let staged_database = temporary.path().join("collection.db");
        drop(Storage::restore_from_backup(&requested, &staged_database)?);
        let media_backup = media_backup_path(&requested);
        let staged_media = temporary.path().join("media");
        let has_media_backup = media_backup.exists();
        if has_media_backup {
            drop(meiki_media::MediaStore::restore_from_backup(
                &media_backup,
                &staged_media,
            )?);
        }

        let current = self.open_storage()?;
        let recovery = self.create_recovery_backup(&current, "pre-restore")?;
        if has_media_backup {
            self.media_store().merge_from_backup(&staged_media)?;
        }
        drop(current);
        drop(Storage::replace_from_backup(
            &staged_database,
            &self.collection_path,
        )?);
        let byte_size = fs::metadata(&recovery)
            .map_err(ApplicationError::PortableIo)?
            .len();
        let recovery_name = recovery
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("recovery.bak")
            .to_owned();
        Ok(BackupDto {
            path: recovery.to_string_lossy().into_owned(),
            file_name: recovery_name,
            byte_size,
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

fn validate_export_request(request: &ArchiveExportRequest) -> Result<(), ApplicationError> {
    if request.now_ms < 0 {
        return Err(ApplicationError::InvalidPortable(
            "archive export timestamp is invalid".into(),
        ));
    }
    Ok(())
}

fn build_collection(storage: &Storage) -> Result<PortableCollection, ApplicationError> {
    let mut decks = storage.list_decks()?;
    let mut notes = storage.library_notes()?;

    decks.sort_by(|left, right| left.id.cmp(&right.id));
    notes.sort_by(|left, right| left.note.source_item.id.cmp(&right.note.source_item.id));
    let mut portable_notes = Vec::with_capacity(notes.len());
    let mut parameter_ids = HashSet::new();
    for stored in notes {
        let mut cards = Vec::with_capacity(stored.cards.len());
        for card in stored.cards {
            let mut review_events = storage.review_events(&card.card.id)?;
            review_events.sort_by(|left, right| {
                left.previous_schedule
                    .version
                    .cmp(&right.previous_schedule.version)
                    .then_with(|| left.id.cmp(&right.id))
            });
            parameter_ids.extend(
                review_events
                    .iter()
                    .filter_map(|event| event.scheduler_parameter_set_id.clone()),
            );
            cards.push(PortableCard {
                baseline: storage.load_schedule_baseline(&card.card.id)?,
                schedule: card.schedule,
                card: card.card,
                review_events,
            });
        }
        cards.sort_by(|left, right| left.card.id.cmp(&right.card.id));
        let mut clozes = stored.note.clozes;
        clozes.sort_by(|left, right| left.id.cmp(&right.id));
        portable_notes.push(PortableNote {
            source_item: stored.note.source_item,
            clozes,
            cards,
            deleted_at_ms: stored.deleted_at_ms,
        });
    }

    let mut scheduler_profiles = decks
        .iter()
        .map(|deck| storage.get_scheduler_profile(&deck.id))
        .collect::<Result<Vec<_>, _>>()?;
    scheduler_profiles.sort_by(|left, right| left.deck_id.cmp(&right.deck_id));
    for profile in &scheduler_profiles {
        parameter_ids.insert(profile.active_parameter_set_id.clone());
    }
    let mut scheduler_parameter_sets = parameter_ids
        .iter()
        .map(|id| storage.get_scheduler_parameter_set(id))
        .collect::<Result<Vec<_>, _>>()?;
    scheduler_parameter_sets.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(PortableCollection {
        collection_scheduling_settings: storage.collection_scheduling_settings()?,
        decks,
        notes: portable_notes,
        scheduler_parameter_sets,
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

fn pristine_archive_deck(collection: &PortableCollection) -> Result<&meiki_domain::Deck, String> {
    let [deck] = collection.decks.as_slice() else {
        return Err("Add deck requires an archive containing exactly one deck.".into());
    };
    if collection
        .notes
        .iter()
        .any(|note| note.deleted_at_ms.is_some())
    {
        return Err("Add deck is unavailable because the archive contains trashed notes.".into());
    }
    for portable in collection.notes.iter().flat_map(|note| note.cards.iter()) {
        if !portable.review_events.is_empty() {
            return Err(
                "Add deck is unavailable because the archive contains review history.".into(),
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
                "Add deck is unavailable because the archive contains scheduled or modified cards."
                    .into(),
            );
        }
    }
    Ok(deck)
}

fn build_pristine_deck_import(
    collection: &PortableCollection,
    imported_at_ms: i64,
) -> PristineDeckImport {
    let mut deck = collection.decks[0].clone();
    deck.settings = StudySettingsOverride::default();
    deck.created_at_ms = imported_at_ms;
    deck.updated_at_ms = imported_at_ms;

    let notes = collection
        .notes
        .iter()
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

fn preview_validated_archive(
    service: &ApplicationService,
    path: &str,
    archive: &ValidatedArchive,
) -> Result<PortableArchivePreviewDto, ApplicationError> {
    let replacement_is_full = archive.manifest.scope == ArchiveScope::FullCollection;
    let store = service.media_store();
    let duplicate_media_objects = archive
        .media_objects
        .iter()
        .filter_map(|media| match store.resolve(&media.content_hash) {
            Ok(_) => Some(Ok(())),
            Err(meiki_media::MediaError::MissingObject(_)) => None,
            Err(error) => Some(Err(ApplicationError::Media(error))),
        })
        .collect::<Result<Vec<_>, _>>()?
        .len();
    let duplicate_media_objects = u64::try_from(duplicate_media_objects)
        .map_err(|_| ApplicationError::NumericRange("duplicate media object count"))?;
    let deck_name =
        (archive.collection.decks.len() == 1).then(|| archive.collection.decks[0].name.clone());
    let (can_add_deck, add_deck_summary) = match pristine_archive_deck(&archive.collection) {
        Ok(deck) => {
            let pristine_import = build_pristine_deck_import(&archive.collection, 0);
            let storage = service.open_storage()?;
            match storage.validate_pristine_deck_import(&pristine_import) {
                Ok(PristineDeckImportStatus::Ready) => (
                    true,
                    format!(
                        "Ready to add deck {:?} with {} note(s), {} card(s), and {} media object(s).",
                        deck.name,
                        archive.manifest.counts.notes,
                        archive.manifest.counts.cards,
                        archive.manifest.counts.media_objects
                    ),
                ),
                Ok(PristineDeckImportStatus::AlreadyInstalled) => {
                    (false, "Deck already installed".into())
                }
                Err(StorageError::InvalidAggregate(reason)) => {
                    (false, format!("Cannot add deck: {reason}"))
                }
                Err(error) => return Err(ApplicationError::Storage(error)),
            }
        }
        Err(reason) => (false, reason),
    };
    let can_import = replacement_is_full;
    let summary = if replacement_is_full {
        format!(
            "Validated {} note(s), {} card(s), and {} media object(s).",
            archive.manifest.counts.notes,
            archive.manifest.counts.cards,
            archive.manifest.counts.media_objects
        )
    } else {
        "Only a full-collection archive can replace the current collection.".into()
    };
    Ok(PortableArchivePreviewDto {
        path: path.to_owned(),
        format_version: archive.manifest.version,
        deck_name,
        decks: archive.manifest.counts.decks,
        notes: archive.manifest.counts.notes,
        cards: archive.manifest.counts.cards,
        review_events: archive.manifest.counts.review_events,
        media_objects: archive.manifest.counts.media_objects,
        duplicate_media_objects,
        can_add_deck,
        add_deck_summary,
        can_import,
        confirmation: REPLACE_CONFIRMATION.into(),
        summary,
    })
}

fn populate_staging(
    storage: &mut Storage,
    collection: &PortableCollection,
) -> Result<(), ApplicationError> {
    storage.update_collection_scheduling_settings(&collection.collection_scheduling_settings)?;
    storage.delete_deck(DEFAULT_DECK_ID)?;
    storage.delete_scheduler_parameter_set(DEFAULT_SCHEDULER_PARAMETER_SET_ID)?;
    for parameter_set in &collection.scheduler_parameter_sets {
        storage.create_scheduler_parameter_set(parameter_set)?;
    }
    for deck in &collection.decks {
        storage.create_deck(deck)?;
    }
    for profile in &collection.scheduler_profiles {
        storage.update_scheduler_profile(profile)?;
    }
    for note in &collection.notes {
        storage.create_source_note(&StoredSourceNote {
            source_item: note.source_item.clone(),
            clozes: note.clozes.clone(),
        })?;
        for portable in &note.cards {
            storage.create_card(&portable.card, &portable.baseline)?;
            storage.restore_card_history(
                &portable.card.id,
                &portable.baseline,
                &portable.schedule,
                &portable.review_events,
            )?;
        }
        if note.deleted_at_ms.is_some() {
            storage.set_library_notes_deleted(
                std::slice::from_ref(&note.source_item.id),
                note.deleted_at_ms,
                note.source_item.updated_at_ms,
            )?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct ArchiveMediaImport {
    imported: u64,
    deduplicated: u64,
    new_hashes: Vec<String>,
}

fn import_archive_media(
    archive: &ValidatedArchive,
    store: &meiki_media::MediaStore,
) -> Result<ArchiveMediaImport, ApplicationError> {
    let mut import = ArchiveMediaImport::default();
    for media in &archive.media_objects {
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

    use meiki_domain::{CardLifecycle, SchedulerParameterSet, SchedulingMode};
    use meiki_media::{DetectedMediaKind, ImportedMedia, MediaError};
    use meiki_portable::{ArchiveMediaSource, PortableCollection, read_archive, write_archive};
    use meiki_storage::{
        CardRepository, DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository,
        SAMPLE_CARD_ID, SAMPLE_SOURCE_ID, SchedulerParameterSetRepository,
        SchedulerProfileRepository, SourceNoteRepository, Storage,
    };
    use tempfile::tempdir;

    use super::{
        ArchiveAddDeckRequest, ArchiveExportRequest, ArchiveImportRequest, build_collection,
        imported_media_metadata_matches,
    };
    use crate::{
        ApplicationService, BudgetSourceDto, CheckAnswerRequest, GradeDto, GradeReviewRequest,
        MediaRoleDto, SchedulingModeDto, TodayRequest, UpdateSchedulerSettingsRequest,
    };

    const FIXTURE_DECK_ID: &str = "fixture-deck-ja-foundation";
    const FIXTURE_SOURCE_ID: &str = "fixture-source-ja-001";
    const FIXTURE_CARD_ID: &str = "fixture-card-ja-001";
    const IMPORTED_AT_MS: i64 = 300_000;
    const PRISTINE_FIXTURE: &[u8] = include_bytes!("../fixtures/pristine-deck-v4.meiki");

    fn copy_pristine_fixture(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        std::fs::write(&path, PRISTINE_FIXTURE).unwrap();
        path
    }

    fn write_fixture_variant(
        directory: &Path,
        name: &str,
        mutate: impl FnOnce(&mut PortableCollection),
    ) -> PathBuf {
        let fixture_path = copy_pristine_fixture(directory, &format!("{name}-source.meiki"));
        let archive = read_archive(&fixture_path).unwrap();
        let mut collection = archive.collection.clone();
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
    #[allow(clippy::too_many_lines)]
    fn pristine_deck_add_preserves_collection_and_uses_local_policy_and_media() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let card = service.seed_test_collection(100_000).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-before-deck-add".into(),
                    card_id: card.card_id.clone(),
                    card_content_version: card.card_content_version,
                    schedule_version: card.schedule_version,
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
                scheduling_mode: SchedulingModeDto::Expert,
                collection_daily_time_budget_minutes: 1,
                deck_daily_time_budget_minutes: None,
                target_retention_basis_points: 9_500,
                new_cards_per_day: 50,
                maximum_interval_days: 20_000,
                day_boundary_minutes: 240,
                now_ms: 110_000,
                day_start_ms: 0,
            })
            .unwrap();

        let before = {
            let storage = Storage::open(&collection_path).unwrap();
            build_collection(&storage).unwrap()
        };
        let archive_path = write_fixture_variant(
            directory.path(),
            "pristine-with-ignored-policy",
            |collection| {
                collection
                    .collection_scheduling_settings
                    .daily_time_budget_minutes = 777;
                let profile = &mut collection.scheduler_profiles[0];
                profile.scheduling_mode = SchedulingMode::Expert;
                profile.deck_daily_time_budget_minutes = Some(321);
                profile.controller_review_count = 44;
                profile.controller_unseen_count = 55;
                profile.controller_backlog_exceeds_budget = true;
                profile.controller_explanation = "archived diagnostic".into();
                let mut extra = collection.scheduler_parameter_sets[0].clone();
                extra.id = "archived-default-copy".into();
                collection
                    .scheduler_parameter_sets
                    .push(SchedulerParameterSet { ..extra });
            },
        );
        let archive = read_archive(&archive_path).unwrap();
        let imported_fixture_media = service
            .media_store()
            .import_file(&archive.media_objects[0].path)
            .unwrap();
        assert!(!imported_fixture_media.deduplicated);
        drop(archive);

        let preview = service
            .preview_archive(&archive_path.to_string_lossy())
            .unwrap();
        assert!(preview.can_add_deck);
        assert!(preview.can_import);
        assert_eq!(
            preview.deck_name.as_deref(),
            Some("Japanese Foundation Fixture")
        );
        assert_eq!(
            (preview.notes, preview.cards, preview.media_objects),
            (1, 1, 1)
        );
        assert_eq!(preview.duplicate_media_objects, 1);

        let added = service
            .add_archive_deck(&ArchiveAddDeckRequest {
                path: archive_path.to_string_lossy().into_owned(),
                now_ms: IMPORTED_AT_MS,
            })
            .unwrap();
        assert_eq!(added.deck_id, FIXTURE_DECK_ID);
        assert_eq!(added.imported_notes, 1);
        assert_eq!(added.imported_cards, 1);
        assert_eq!(added.imported_media_objects, 0);
        assert_eq!(added.deduplicated_media_objects, 1);
        assert!(Path::new(&added.backup_path).is_file());
        assert_eq!(service.list_backups().unwrap().len(), 1);

        let storage = Storage::open(&collection_path).unwrap();
        let after = build_collection(&storage).unwrap();
        assert_eq!(
            after
                .notes
                .iter()
                .find(|note| note.source_item.id == SAMPLE_SOURCE_ID),
            before
                .notes
                .iter()
                .find(|note| note.source_item.id == SAMPLE_SOURCE_ID)
        );
        assert_eq!(
            after.decks.iter().find(|deck| deck.id == DEFAULT_DECK_ID),
            before.decks.iter().find(|deck| deck.id == DEFAULT_DECK_ID)
        );
        assert_eq!(
            storage.review_events(SAMPLE_CARD_ID).unwrap(),
            before.notes[0].cards[0].review_events
        );
        assert_eq!(
            after.collection_scheduling_settings,
            before.collection_scheduling_settings
        );
        assert_eq!(
            storage.get_scheduler_profile(DEFAULT_DECK_ID).unwrap(),
            before.scheduler_profiles[0]
        );
        assert_eq!(
            storage
                .get_scheduler_parameter_set(DEFAULT_SCHEDULER_PARAMETER_SET_ID)
                .unwrap(),
            before.scheduler_parameter_sets[0]
        );
        assert!(
            storage
                .get_scheduler_parameter_set("archived-default-copy")
                .is_err()
        );

        let imported_deck = storage.get_deck(FIXTURE_DECK_ID).unwrap();
        assert_eq!(imported_deck.created_at_ms, IMPORTED_AT_MS);
        assert_eq!(imported_deck.updated_at_ms, IMPORTED_AT_MS);
        assert_eq!(
            imported_deck.settings,
            meiki_domain::StudySettingsOverride::default()
        );
        let imported_note = storage.get_source_note(FIXTURE_SOURCE_ID).unwrap();
        assert_eq!(imported_note.source_item.created_at_ms, IMPORTED_AT_MS);
        assert_eq!(imported_note.source_item.updated_at_ms, IMPORTED_AT_MS);
        assert!(
            imported_note
                .source_item
                .media
                .iter()
                .all(|media| media.created_at_ms == IMPORTED_AT_MS)
        );
        assert_eq!(imported_note.clozes[0].created_at_ms, IMPORTED_AT_MS);
        assert_eq!(imported_note.clozes[0].updated_at_ms, IMPORTED_AT_MS);
        let imported_card = storage.get_card(FIXTURE_CARD_ID).unwrap();
        assert_eq!(imported_card.content_version, 1);
        assert_eq!(imported_card.created_at_ms, IMPORTED_AT_MS);
        assert_eq!(imported_card.updated_at_ms, IMPORTED_AT_MS);
        let schedule = storage.load_schedule(FIXTURE_CARD_ID).unwrap();
        assert_eq!(schedule.version, 0);
        assert_eq!(schedule.lifecycle, CardLifecycle::Unseen);
        assert_eq!(schedule.due_at_ms, IMPORTED_AT_MS);
        assert_eq!(schedule.ideal_due_at_ms, IMPORTED_AT_MS);
        let imported_portable = after
            .notes
            .iter()
            .find(|note| note.source_item.id == FIXTURE_SOURCE_ID)
            .unwrap();
        assert_eq!(
            imported_portable.cards[0].baseline,
            imported_portable.cards[0].schedule
        );
        drop(storage);

        let imported_settings = service.get_scheduler_settings(FIXTURE_DECK_ID).unwrap();
        assert_eq!(
            imported_settings.scheduling_mode,
            SchedulingModeDto::Automatic
        );
        assert_eq!(imported_settings.collection_daily_time_budget_minutes, 1);
        assert_eq!(imported_settings.deck_daily_time_budget_minutes, None);
        assert_eq!(
            imported_settings.budget_source,
            BudgetSourceDto::CollectionBudget
        );
        let today = service
            .get_today_overview(&TodayRequest {
                deck_id: FIXTURE_DECK_ID.into(),
                now_ms: IMPORTED_AT_MS,
                day_start_ms: IMPORTED_AT_MS,
                day_end_ms: IMPORTED_AT_MS + 86_400_000,
            })
            .unwrap();
        assert_eq!(today.daily_time_budget_minutes, Some(1));
        assert_eq!(today.budget_source, BudgetSourceDto::CollectionBudget);
        assert_eq!(today.new_cards + today.deferred_new_cards, 1);
        assert_eq!(today.queue.len(), usize::try_from(today.new_cards).unwrap());
        assert!(
            today
                .queue
                .iter()
                .all(|queued| queued.card_id == FIXTURE_CARD_ID)
        );

        let study = service.get_study_card(FIXTURE_CARD_ID).unwrap();
        assert_eq!(study.prompt_media.len(), 1);
        assert_eq!(study.prompt_media[0].role, MediaRoleDto::PromptAudio);
        let reveal = service
            .check_answer(&CheckAnswerRequest {
                card_id: study.card_id,
                card_content_version: study.card_content_version,
                schedule_version: study.schedule_version,
                raw_response: "晴れです".into(),
            })
            .unwrap();
        assert_eq!(reveal.answer_media.len(), 1);
        assert_eq!(reveal.answer_media[0].role, MediaRoleDto::AnswerAudio);
        assert_eq!(
            reveal.answer_media[0].content_hash,
            study.prompt_media[0].content_hash
        );

        let repeat_preview = service
            .preview_archive(&archive_path.to_string_lossy())
            .unwrap();
        assert!(!repeat_preview.can_add_deck);
        assert_eq!(repeat_preview.add_deck_summary, "Deck already installed");
        assert!(
            service
                .add_archive_deck(&ArchiveAddDeckRequest {
                    path: archive_path.to_string_lossy().into_owned(),
                    now_ms: IMPORTED_AT_MS + 1,
                })
                .unwrap_err()
                .to_string()
                .contains("Deck already installed")
        );
        assert_eq!(service.list_backups().unwrap().len(), 1);
        assert_eq!(
            Storage::open(&collection_path)
                .unwrap()
                .list_decks()
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn add_deck_rejects_reviewed_scheduled_trashed_and_malformed_archives() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);

        let reviewed_path = {
            let source = ApplicationService::new(directory.path().join("reviewed.db"));
            let card = source.seed_test_collection(1_000).unwrap();
            source
                .grade_review_at(
                    &GradeReviewRequest {
                        review_event_id: "fixture-review".into(),
                        card_id: card.card_id,
                        card_content_version: card.card_content_version,
                        schedule_version: card.schedule_version,
                        raw_response: "行きます".into(),
                        chosen_grade: GradeDto::Good,
                        response_duration_ms: 1_000,
                    },
                    1_000,
                )
                .unwrap();
            PathBuf::from(
                source
                    .export_archive(&ArchiveExportRequest { now_ms: 2_000 })
                    .unwrap()
                    .path,
            )
        };
        let scheduled_path = write_fixture_variant(directory.path(), "scheduled", |collection| {
            let card = &mut collection.notes[0].cards[0];
            card.baseline.lifecycle = CardLifecycle::Introduced;
            card.schedule.lifecycle = CardLifecycle::Introduced;
        });
        let trashed_path = write_fixture_variant(directory.path(), "trashed", |collection| {
            collection.notes[0].deleted_at_ms = Some(9_000);
        });
        let malformed_path = directory.path().join("malformed.meiki");
        std::fs::write(&malformed_path, b"not a portable archive").unwrap();

        for (path, expected) in [
            (reviewed_path, "review history"),
            (scheduled_path, "scheduled or modified"),
            (trashed_path, "trashed notes"),
        ] {
            let preview = service.preview_archive(&path.to_string_lossy()).unwrap();
            assert!(!preview.can_add_deck);
            assert!(preview.add_deck_summary.contains(expected));
            assert!(
                service
                    .add_archive_deck(&ArchiveAddDeckRequest {
                        path: path.to_string_lossy().into_owned(),
                        now_ms: IMPORTED_AT_MS,
                    })
                    .is_err()
            );
        }
        assert!(
            service
                .add_archive_deck(&ArchiveAddDeckRequest {
                    path: malformed_path.to_string_lossy().into_owned(),
                    now_ms: IMPORTED_AT_MS,
                })
                .is_err()
        );
        assert!(service.list_backups().unwrap().is_empty());
        assert_eq!(service.list_decks().unwrap().len(), 1);
    }

    #[test]
    fn add_deck_preflights_identity_collisions_without_writes() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        service.seed_test_collection(1_000).unwrap();
        let collision_path =
            write_fixture_variant(directory.path(), "card-collision", |collection| {
                let card = &mut collection.notes[0].cards[0];
                card.card.id = SAMPLE_CARD_ID.into();
                card.baseline.card_id = SAMPLE_CARD_ID.into();
                card.schedule.card_id = SAMPLE_CARD_ID.into();
            });

        let preview = service
            .preview_archive(&collision_path.to_string_lossy())
            .unwrap();
        assert!(!preview.can_add_deck);
        assert!(preview.add_deck_summary.contains("card"));
        assert!(preview.add_deck_summary.contains("already exists"));
        assert!(
            service
                .add_archive_deck(&ArchiveAddDeckRequest {
                    path: collision_path.to_string_lossy().into_owned(),
                    now_ms: IMPORTED_AT_MS,
                })
                .is_err()
        );
        assert!(service.list_backups().unwrap().is_empty());
        assert_eq!(
            Storage::open(&collection_path)
                .unwrap()
                .library_notes()
                .unwrap()
                .len(),
            1
        );
        assert!(
            service
                .media_store()
                .resolve("sha256:4f8734c5e13ac599e168cf247a51c1dd0758537ce00bf16d7fed1a3d14d07041")
                .is_err()
        );
    }

    #[test]
    fn add_deck_rolls_back_new_media_when_technical_metadata_is_invalid() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let invalid_media_path =
            write_fixture_variant(directory.path(), "invalid-media", |collection| {
                for media in &mut collection.notes[0].source_item.media {
                    media.media_type = "audio/mpeg".into();
                }
            });
        let hash = read_archive(&invalid_media_path).unwrap().media_objects[0]
            .content_hash
            .clone();
        assert!(
            service
                .preview_archive(&invalid_media_path.to_string_lossy())
                .unwrap()
                .can_add_deck
        );

        let error = service
            .add_archive_deck(&ArchiveAddDeckRequest {
                path: invalid_media_path.to_string_lossy().into_owned(),
                now_ms: IMPORTED_AT_MS,
            })
            .unwrap_err();
        assert!(error.to_string().contains("technical metadata"));
        assert!(matches!(
            service.media_store().resolve(&hash),
            Err(MediaError::MissingObject(_))
        ));
        assert!(
            Storage::open(&collection_path)
                .unwrap()
                .get_deck(FIXTURE_DECK_ID)
                .is_err()
        );
        assert_eq!(service.list_backups().unwrap().len(), 1);
    }

    #[test]
    fn empty_collection_exports_previews_and_replaces_without_learning_data() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);

        let exported = service
            .export_archive(&ArchiveExportRequest { now_ms: 10_000 })
            .unwrap();
        assert_eq!(exported.decks, 1);
        assert_eq!(exported.notes, 0);
        assert_eq!(exported.cards, 0);
        assert_eq!(exported.review_events, 0);
        assert_eq!(exported.media_objects, 0);

        let preview = service.preview_archive(&exported.path).unwrap();
        assert!(preview.can_import);
        assert_eq!(preview.notes, 0);
        assert_eq!(preview.cards, 0);
        assert_eq!(preview.review_events, 0);
        let imported = service
            .import_archive(&ArchiveImportRequest {
                path: exported.path,
                confirmation: "REPLACE".into(),
            })
            .unwrap();
        assert!(std::path::Path::new(&imported.backup_path).is_file());
        assert!(
            Storage::open(&collection_path)
                .unwrap()
                .library_notes()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn repaired_snapshot_history_exports_and_replaces_exactly() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let card = service.seed_test_collection(100_000).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-before-repair".into(),
                    card_id: card.card_id.clone(),
                    card_content_version: card.card_content_version,
                    schedule_version: card.schedule_version,
                    raw_response: "行きます".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 1_000,
                },
                100_000,
            )
            .unwrap();
        let expected = Storage::open(&collection_path)
            .unwrap()
            .load_schedule(SAMPLE_CARD_ID)
            .unwrap();
        let events = Storage::open(&collection_path)
            .unwrap()
            .review_events(SAMPLE_CARD_ID)
            .unwrap();
        service
            .update_scheduler_settings(&UpdateSchedulerSettingsRequest {
                deck_id: meiki_storage::DEFAULT_DECK_ID.into(),
                scheduling_mode: SchedulingModeDto::Expert,
                collection_daily_time_budget_minutes: 120,
                deck_daily_time_budget_minutes: None,
                target_retention_basis_points: 9_500,
                new_cards_per_day: 50,
                maximum_interval_days: 20_000,
                day_boundary_minutes: 240,
                now_ms: 110_000,
                day_start_ms: 0,
            })
            .unwrap();

        let mut storage = Storage::open(&collection_path).unwrap();
        assert!(
            storage
                .check_collection_schedule_integrity()
                .unwrap()
                .is_valid()
        );
        let backup = service
            .create_recovery_backup(&storage, "projection-repair-test")
            .unwrap();
        assert!(backup.is_file());
        assert_eq!(
            storage.rebuild_schedule_projection(SAMPLE_CARD_ID).unwrap(),
            expected
        );
        drop(storage);

        let exported = service
            .export_archive(&ArchiveExportRequest { now_ms: 200_000 })
            .unwrap();
        let target_path = directory.path().join("restored.db");
        let target = ApplicationService::new(&target_path);
        assert!(target.preview_archive(&exported.path).unwrap().can_import);
        target
            .import_archive(&ArchiveImportRequest {
                path: exported.path,
                confirmation: "REPLACE".into(),
            })
            .unwrap();
        let restored = Storage::open(&target_path).unwrap();
        assert_eq!(restored.load_schedule(SAMPLE_CARD_ID).unwrap(), expected);
        assert_eq!(restored.review_events(SAMPLE_CARD_ID).unwrap(), events);
        assert!(
            restored
                .check_collection_schedule_integrity()
                .unwrap()
                .is_valid()
        );
        drop(restored);

        let continued = target
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-after-replacement".into(),
                    card_id: card.card_id,
                    card_content_version: card.card_content_version,
                    schedule_version: u32::try_from(expected.version).unwrap(),
                    raw_response: "行きます".into(),
                    chosen_grade: GradeDto::Good,
                    response_duration_ms: 900,
                },
                expected.due_at_ms,
            )
            .unwrap();
        assert_eq!(
            continued.schedule_version,
            u32::try_from(expected.version + 1).unwrap()
        );
        assert_eq!(
            Storage::open(&target_path)
                .unwrap()
                .review_events(SAMPLE_CARD_ID)
                .unwrap()
                .len(),
            events.len() + 1
        );
    }

    #[test]
    fn full_archive_replacement_creates_recovery_and_restores_exact_content() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        service.seed_test_collection(1_000).unwrap();
        let media_source = directory.path().join("recovery.png");
        std::fs::write(
            &media_source,
            [
                0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D',
                b'R', 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0,
            ],
        )
        .unwrap();
        let media = service.media_store().import_file(&media_source).unwrap();
        let exported = service
            .export_archive(&ArchiveExportRequest { now_ms: 10_000 })
            .unwrap();

        {
            let mut storage = Storage::open(&collection_path).unwrap();
            let mut deck = storage.get_deck("default-deck").unwrap();
            deck.name = "Changed".into();
            storage.update_deck(&deck).unwrap();
        }
        let replace_preview = service.preview_archive(&exported.path).unwrap();
        assert!(replace_preview.can_import);
        let replaced = service
            .import_archive(&ArchiveImportRequest {
                path: exported.path,
                confirmation: "REPLACE".into(),
            })
            .unwrap();
        assert!(std::path::Path::new(&replaced.backup_path).is_file());
        assert!(std::path::Path::new(&format!("{}.media", replaced.backup_path)).is_dir());
        service.media_store().resolve(&media.content_hash).unwrap();
        let restored = Storage::open(&collection_path).unwrap();
        assert_eq!(restored.get_deck("default-deck").unwrap().name, "Default");
        assert_eq!(restored.library_notes().unwrap().len(), 1);
    }

    #[test]
    fn managed_backup_restores_after_a_rejected_replacement() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        service.seed_test_collection(1_000).unwrap();
        let storage = Storage::open(&collection_path).unwrap();
        let backup = service
            .create_recovery_backup(&storage, "before-rejected-import")
            .unwrap();
        drop(storage);

        {
            let mut storage = Storage::open(&collection_path).unwrap();
            let mut deck = storage.get_deck("default-deck").unwrap();
            deck.name = "Changed after backup".into();
            storage.update_deck(&deck).unwrap();
        }
        let invalid_archive = directory.path().join("invalid.meiki");
        std::fs::write(&invalid_archive, b"not a portable archive").unwrap();
        assert!(
            service
                .import_archive(&ArchiveImportRequest {
                    path: invalid_archive.to_string_lossy().into_owned(),
                    confirmation: "REPLACE".into(),
                })
                .is_err()
        );
        assert_eq!(
            Storage::open(&collection_path)
                .unwrap()
                .get_deck("default-deck")
                .unwrap()
                .name,
            "Changed after backup"
        );

        let backup_name = backup.file_name().unwrap().to_str().unwrap();
        service
            .restore_backup(&backup.to_string_lossy(), backup_name)
            .unwrap();
        let restored = Storage::open(&collection_path).unwrap();
        assert_eq!(restored.get_deck("default-deck").unwrap().name, "Default");
        assert_eq!(restored.library_notes().unwrap().len(), 1);
    }
}
