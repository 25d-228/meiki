use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{ApplicationError, ApplicationService};
use meiki_portable::{
    ArchiveMediaSource, ArchiveScope, PortableCard, PortableCollection, PortableNote,
    ValidatedArchive, read_archive, write_archive,
};
use meiki_storage::{
    CardRepository, DEFAULT_DECK_ID, DEFAULT_SCHEDULER_PARAMETER_SET_ID, DeckRepository,
    SchedulerParameterSetRepository, SchedulerProfileRepository, SourceNoteRepository, Storage,
    StoredSourceNote,
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
pub struct PortableArchivePreviewDto {
    pub path: String,
    pub format_version: u32,
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
        decks: archive.manifest.counts.decks,
        notes: archive.manifest.counts.notes,
        cards: archive.manifest.counts.cards,
        review_events: archive.manifest.counts.review_events,
        media_objects: archive.manifest.counts.media_objects,
        duplicate_media_objects,
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

#[cfg(test)]
mod tests {
    use meiki_storage::{DeckRepository, SAMPLE_CARD_ID, Storage};
    use tempfile::tempdir;

    use super::{ArchiveExportRequest, ArchiveImportRequest};
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
