use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use meiki_portable::{
    ArchiveMediaSource, ArchiveScope, PortableCard, PortableCollection, PortableNote,
    ValidatedArchive, namespace_collection, read_archive, write_archive,
};
use meiki_storage::{
    CardRepository, DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository,
    SchedulerParameterSetRepository, SchedulerProfileRepository, SourceNoteRepository, Storage,
    StorageError, StoredSourceNote,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{ApplicationError, ApplicationService};

const BACKUP_RETENTION: usize = 5;
const IMPORT_CONFIRMATION: &str = "IMPORT";
const REPLACE_CONFIRMATION: &str = "REPLACE";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ArchiveScopeDto {
    FullCollection,
    SelectedDecks,
    SelectedNotes,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveExportRequest {
    pub scope: ArchiveScopeDto,
    pub selected_ids: Vec<String>,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum ArchiveImportModeDto {
    Merge,
    Replace,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ArchiveImportRequest {
    pub path: String,
    pub mode: ArchiveImportModeDto,
    pub confirmation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct PortableArchivePreviewDto {
    pub path: String,
    pub format_version: u32,
    pub scope: ArchiveScopeDto,
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
    #[ts(type = "number")]
    pub identity_collisions: u64,
    pub can_import: bool,
    pub confirmation: String,
    pub summary: String,
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
    /// Exports a full collection, selected decks, or selected notes as a
    /// versioned `.meiki` archive.
    ///
    /// # Errors
    ///
    /// Returns an error when selection references are invalid, stored
    /// aggregates are inconsistent, media is unavailable, or writing fails.
    pub fn export_archive(
        &self,
        request: &ArchiveExportRequest,
    ) -> Result<PortableExportResultDto, ApplicationError> {
        validate_export_request(request)?;
        let storage = self.open_storage()?;
        let collection = build_collection(&storage, request)?;
        let media = media_sources(&collection, &self.media_store())?;
        let directory = self.export_directory()?;
        let path = directory.join(format!("meiki-{}-{}.meiki", request.now_ms, Uuid::new_v4()));
        let manifest = write_archive(
            &path,
            &collection,
            &media,
            request.scope.into(),
            request.now_ms,
        )?;
        Ok(PortableExportResultDto {
            path: path.to_string_lossy().into_owned(),
            decks: manifest.counts.decks,
            notes: manifest.counts.notes,
            cards: manifest.counts.cards,
            review_events: manifest.counts.review_events,
            media_objects: manifest.counts.media_objects,
        })
    }

    /// Validates an archive and reports exactly what a merge or replacement
    /// would do without changing the collection.
    ///
    /// # Errors
    ///
    /// Returns an error when the archive or the current media store is invalid.
    pub fn preview_archive(
        &self,
        path: &str,
        mode: ArchiveImportModeDto,
    ) -> Result<PortableArchivePreviewDto, ApplicationError> {
        let archive = read_archive(Path::new(path))?;
        preview_validated_archive(self, path, mode, &archive)
    }

    /// Imports a previously previewable archive through a staging database.
    ///
    /// The current collection is backed up immediately before its database is
    /// atomically replaced. A failed staging operation leaves the live
    /// collection unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid confirmation, archive validation,
    /// deterministic identity collisions, staging, media, backup, or restore.
    pub fn import_archive(
        &self,
        request: &ArchiveImportRequest,
    ) -> Result<ArchiveImportResultDto, ApplicationError> {
        let expected_confirmation = match request.mode {
            ArchiveImportModeDto::Merge => IMPORT_CONFIRMATION,
            ArchiveImportModeDto::Replace => REPLACE_CONFIRMATION,
        };
        if request.confirmation != expected_confirmation {
            return Err(ApplicationError::InvalidPortable(format!(
                "type {expected_confirmation} to confirm this import"
            )));
        }

        let archive = read_archive(Path::new(&request.path))?;
        let preview = preview_validated_archive(self, &request.path, request.mode, &archive)?;
        if !preview.can_import {
            return Err(ApplicationError::InvalidPortable(preview.summary));
        }
        let collection = match request.mode {
            ArchiveImportModeDto::Merge => {
                namespace_collection(&archive.collection, &archive.manifest.collection_sha256)?
            }
            ArchiveImportModeDto::Replace => archive.collection.clone(),
        };

        let temporary = tempfile::tempdir().map_err(ApplicationError::PortableIo)?;
        let staging_path = temporary.path().join("collection.db");
        let current = self.open_storage()?;
        let mut staging = match request.mode {
            ArchiveImportModeDto::Merge => {
                current.backup_to(&staging_path)?;
                Storage::open(&staging_path)?
            }
            ArchiveImportModeDto::Replace => Storage::open(&staging_path)?,
        };
        populate_staging(
            &mut staging,
            &collection,
            request.mode == ArchiveImportModeDto::Replace,
        )?;

        let backup_path = self.create_recovery_backup(&current, "pre-import")?;
        let (imported_media_objects, deduplicated_media_objects) =
            import_archive_media(&archive, &self.media_store())?;
        drop(staging);
        drop(current);
        drop(Storage::replace_from_backup(
            &staging_path,
            &self.collection_path,
        )?);

        Ok(ArchiveImportResultDto {
            backup_path: backup_path.to_string_lossy().into_owned(),
            imported_notes: archive.manifest.counts.notes,
            imported_cards: archive.manifest.counts.cards,
            imported_media_objects,
            deduplicated_media_objects,
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
    let unique = request.selected_ids.iter().collect::<HashSet<_>>();
    let requires_selection = request.scope != ArchiveScopeDto::FullCollection;
    if request.now_ms < 0
        || unique.len() != request.selected_ids.len()
        || request.selected_ids.iter().any(|id| id.trim().is_empty())
        || (requires_selection && request.selected_ids.is_empty())
        || (!requires_selection && !request.selected_ids.is_empty())
    {
        return Err(ApplicationError::InvalidPortable(
            "archive export scope, selection, or timestamp is invalid".into(),
        ));
    }
    Ok(())
}

fn build_collection(
    storage: &Storage,
    request: &ArchiveExportRequest,
) -> Result<PortableCollection, ApplicationError> {
    let selected = request.selected_ids.iter().collect::<HashSet<_>>();
    let mut decks = storage.list_decks()?;
    let mut notes = storage.library_notes()?;

    match request.scope {
        ArchiveScopeDto::FullCollection => {}
        ArchiveScopeDto::SelectedDecks => {
            let existing = decks.iter().map(|deck| &deck.id).collect::<HashSet<_>>();
            if !selected.is_subset(&existing) {
                return Err(ApplicationError::InvalidPortable(
                    "one or more selected decks no longer exist".into(),
                ));
            }
            decks.retain(|deck| selected.contains(&deck.id));
            notes.retain(|note| selected.contains(&note.note.source_item.deck_id));
        }
        ArchiveScopeDto::SelectedNotes => {
            let existing = notes
                .iter()
                .map(|note| &note.note.source_item.id)
                .collect::<HashSet<_>>();
            if !selected.is_subset(&existing) {
                return Err(ApplicationError::InvalidPortable(
                    "one or more selected notes no longer exist".into(),
                ));
            }
            notes.retain(|note| selected.contains(&note.note.source_item.id));
            let deck_ids = notes
                .iter()
                .map(|note| note.note.source_item.deck_id.as_str())
                .collect::<HashSet<_>>();
            decks.retain(|deck| deck_ids.contains(deck.id.as_str()));
        }
    }

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
        if let Some(previous) = &profile.previous_parameter_set_id {
            parameter_ids.insert(previous.clone());
        }
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

fn preview_validated_archive(
    service: &ApplicationService,
    path: &str,
    mode: ArchiveImportModeDto,
    archive: &ValidatedArchive,
) -> Result<PortableArchivePreviewDto, ApplicationError> {
    let replacement_is_full = archive.manifest.scope == ArchiveScope::FullCollection;
    let collection = match mode {
        ArchiveImportModeDto::Merge => {
            namespace_collection(&archive.collection, &archive.manifest.collection_sha256)?
        }
        ArchiveImportModeDto::Replace => archive.collection.clone(),
    };
    let identity_collisions = if mode == ArchiveImportModeDto::Merge {
        let storage = service.open_storage()?;
        identity_collision_count(&storage, &collection)?
    } else {
        0
    };
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
    let can_import =
        identity_collisions == 0 && (mode == ArchiveImportModeDto::Merge || replacement_is_full);
    let confirmation = match mode {
        ArchiveImportModeDto::Merge => IMPORT_CONFIRMATION,
        ArchiveImportModeDto::Replace => REPLACE_CONFIRMATION,
    };
    let summary = if mode == ArchiveImportModeDto::Replace && !replacement_is_full {
        "Only a full-collection archive can replace the current collection.".into()
    } else if identity_collisions > 0 {
        format!(
            "{identity_collisions} deterministic identity collision(s) prevent this import; this archive may already be imported."
        )
    } else {
        format!(
            "Validated {} note(s), {} card(s), and {} media object(s).",
            archive.manifest.counts.notes,
            archive.manifest.counts.cards,
            archive.manifest.counts.media_objects
        )
    };
    Ok(PortableArchivePreviewDto {
        path: path.to_owned(),
        format_version: archive.manifest.version,
        scope: archive.manifest.scope.clone().into(),
        decks: archive.manifest.counts.decks,
        notes: archive.manifest.counts.notes,
        cards: archive.manifest.counts.cards,
        review_events: archive.manifest.counts.review_events,
        media_objects: archive.manifest.counts.media_objects,
        duplicate_media_objects,
        identity_collisions,
        can_import,
        confirmation: confirmation.into(),
        summary,
    })
}

fn identity_collision_count(
    storage: &Storage,
    collection: &PortableCollection,
) -> Result<u64, ApplicationError> {
    let mut collisions = 0_u64;
    for deck in &collection.decks {
        collisions += u64::from(entity_exists(storage.get_deck(&deck.id))?);
    }
    for parameter_set in &collection.scheduler_parameter_sets {
        collisions += u64::from(entity_exists(
            storage.get_scheduler_parameter_set(&parameter_set.id),
        )?);
    }
    for note in &collection.notes {
        collisions += u64::from(entity_exists(
            storage.get_source_note(&note.source_item.id),
        )?);
        for card in &note.cards {
            collisions += u64::from(entity_exists(storage.get_card(&card.card.id))?);
        }
    }
    Ok(collisions)
}

fn entity_exists<T>(result: Result<T, StorageError>) -> Result<bool, ApplicationError> {
    match result {
        Ok(_) => Ok(true),
        Err(StorageError::EntityNotFound { .. } | StorageError::CardNotFound(_)) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn populate_staging(
    storage: &mut Storage,
    collection: &PortableCollection,
    replace: bool,
) -> Result<(), ApplicationError> {
    if replace {
        storage
            .update_collection_scheduling_settings(&collection.collection_scheduling_settings)?;
    }
    if replace {
        storage.delete_deck(DEFAULT_DECK_ID)?;
        storage.delete_scheduler_parameter_set(DEFAULT_SCHEDULER_PARAMETER_SET_ID)?;
    }
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

fn import_archive_media(
    archive: &ValidatedArchive,
    store: &meiki_media::MediaStore,
) -> Result<(u64, u64), ApplicationError> {
    let mut imported = 0_u64;
    let mut deduplicated = 0_u64;
    for media in &archive.media_objects {
        let result = store.import_file(&media.path)?;
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
            let detected_kind = match result.kind {
                meiki_media::DetectedMediaKind::Audio => meiki_domain::MediaKind::Audio,
                meiki_media::DetectedMediaKind::Image => meiki_domain::MediaKind::Image,
            };
            if reference.kind != detected_kind
                || reference.media_type != result.media_type
                || reference.byte_size != result.byte_size
                || reference.width != result.width
                || reference.height != result.height
                || reference.duration_ms != result.duration_ms
            {
                return Err(ApplicationError::InvalidPortable(format!(
                    "media technical metadata does not match {}",
                    media.content_hash
                )));
            }
        }
        if result.deduplicated {
            deduplicated += 1;
        } else {
            imported += 1;
        }
    }
    Ok((imported, deduplicated))
}

impl From<ArchiveScopeDto> for ArchiveScope {
    fn from(value: ArchiveScopeDto) -> Self {
        match value {
            ArchiveScopeDto::FullCollection => Self::FullCollection,
            ArchiveScopeDto::SelectedDecks => Self::SelectedDecks,
            ArchiveScopeDto::SelectedNotes => Self::SelectedNotes,
        }
    }
}

impl From<ArchiveScope> for ArchiveScopeDto {
    fn from(value: ArchiveScope) -> Self {
        match value {
            ArchiveScope::FullCollection => Self::FullCollection,
            ArchiveScope::SelectedDecks => Self::SelectedDecks,
            ArchiveScope::SelectedNotes => Self::SelectedNotes,
        }
    }
}

#[cfg(test)]
mod tests {
    use meiki_storage::{DeckRepository, SAMPLE_CARD_ID, Storage};
    use tempfile::tempdir;

    use super::{
        ArchiveExportRequest, ArchiveImportModeDto, ArchiveImportRequest, ArchiveScopeDto,
    };
    use crate::{
        ApplicationService, GradeDto, GradeReviewRequest, SchedulingModeDto,
        UpdateSchedulerSettingsRequest,
    };

    #[test]
    fn empty_collection_exports_previews_and_replaces_without_learning_data() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);

        let exported = service
            .export_archive(&ArchiveExportRequest {
                scope: ArchiveScopeDto::FullCollection,
                selected_ids: Vec::new(),
                now_ms: 10_000,
            })
            .unwrap();
        assert_eq!(exported.decks, 1);
        assert_eq!(exported.notes, 0);
        assert_eq!(exported.cards, 0);
        assert_eq!(exported.review_events, 0);
        assert_eq!(exported.media_objects, 0);

        let preview = service
            .preview_archive(&exported.path, ArchiveImportModeDto::Replace)
            .unwrap();
        assert!(preview.can_import);
        assert_eq!(preview.notes, 0);
        assert_eq!(preview.cards, 0);
        assert_eq!(preview.review_events, 0);
        let imported = service
            .import_archive(&ArchiveImportRequest {
                path: exported.path,
                mode: ArchiveImportModeDto::Replace,
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
    fn repaired_snapshot_history_exports_and_replaces_exactly() {
        let directory = tempdir().unwrap();
        let collection_path = directory.path().join("collection.db");
        let service = ApplicationService::new(&collection_path);
        let card = service.seed_test_collection(100_000).unwrap();
        service
            .grade_review_at(
                &GradeReviewRequest {
                    review_event_id: "review-before-repair".into(),
                    card_id: card.card_id,
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
            .export_archive(&ArchiveExportRequest {
                scope: ArchiveScopeDto::FullCollection,
                selected_ids: Vec::new(),
                now_ms: 200_000,
            })
            .unwrap();
        let target_path = directory.path().join("restored.db");
        let target = ApplicationService::new(&target_path);
        assert!(
            target
                .preview_archive(&exported.path, ArchiveImportModeDto::Replace)
                .unwrap()
                .can_import
        );
        target
            .import_archive(&ArchiveImportRequest {
                path: exported.path,
                mode: ArchiveImportModeDto::Replace,
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
    }

    #[test]
    fn full_archive_merges_once_and_replacement_restores_exact_content() {
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
            .export_archive(&ArchiveExportRequest {
                scope: ArchiveScopeDto::FullCollection,
                selected_ids: Vec::new(),
                now_ms: 10_000,
            })
            .unwrap();

        let merge_preview = service
            .preview_archive(&exported.path, ArchiveImportModeDto::Merge)
            .unwrap();
        assert!(merge_preview.can_import);
        let merged = service
            .import_archive(&ArchiveImportRequest {
                path: exported.path.clone(),
                mode: ArchiveImportModeDto::Merge,
                confirmation: "IMPORT".into(),
            })
            .unwrap();
        assert!(std::path::Path::new(&merged.backup_path).is_file());
        assert!(std::path::Path::new(&format!("{}.media", merged.backup_path)).is_dir());
        assert_eq!(
            Storage::open(&collection_path)
                .unwrap()
                .library_notes()
                .unwrap()
                .len(),
            2
        );
        let repeated = service
            .preview_archive(&exported.path, ArchiveImportModeDto::Merge)
            .unwrap();
        assert!(!repeated.can_import);
        assert!(repeated.identity_collisions > 0);
        service.media_store().remove(&media.content_hash).unwrap();
        let backup_name = std::path::Path::new(&merged.backup_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        service
            .restore_backup(&merged.backup_path, backup_name)
            .unwrap();
        service.media_store().resolve(&media.content_hash).unwrap();

        {
            let mut storage = Storage::open(&collection_path).unwrap();
            let mut deck = storage.get_deck("default-deck").unwrap();
            deck.name = "Changed".into();
            storage.update_deck(&deck).unwrap();
        }
        let replace_preview = service
            .preview_archive(&exported.path, ArchiveImportModeDto::Replace)
            .unwrap();
        assert!(replace_preview.can_import);
        service
            .import_archive(&ArchiveImportRequest {
                path: exported.path,
                mode: ArchiveImportModeDto::Replace,
                confirmation: "REPLACE".into(),
            })
            .unwrap();
        let restored = Storage::open(&collection_path).unwrap();
        assert_eq!(restored.get_deck("default-deck").unwrap().name, "Default");
        assert_eq!(restored.library_notes().unwrap().len(), 1);
    }
}
