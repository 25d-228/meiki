//! `SQLite` implementation of Meiki persistence.
//!
//! SQL is owned by this crate and must not leak through its public interface.

mod repository;
mod review;

pub use repository::{
    AnnotationRepository, CardRepository, ClozeRepository, DeckRepository, MediaRepository,
    PristineDeckRepository, SchedulerParameterSetRepository, SchedulerProfileRepository,
    SourceNoteRepository, TagRepository,
};

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use meiki_domain::{Card, Cloze, Deck, Direction, MatchingPolicy, ScheduleState, SourceItem};
#[cfg(any(test, feature = "test-fixtures"))]
use rusqlite::params;
use rusqlite::{Connection, MAIN_DB, OptionalExtension};
use thiserror::Error;

const FOUNDATION_MIGRATION: &str = include_str!("../migrations/0001_foundation.sql");
const CORE_MODEL_MIGRATION: &str = include_str!("../migrations/0002_core_model.sql");
const AUTHORING_DEFAULTS_MIGRATION: &str =
    include_str!("../migrations/0003_authoring_defaults.sql");
const FSRS7_SCHEDULER_MIGRATION: &str = include_str!("../migrations/0004_fsrs7_scheduler.sql");
const STUDY_SESSION_MIGRATION: &str = include_str!("../migrations/0005_study_session.sql");
const MEDIA_PIPELINE_MIGRATION: &str = include_str!("../migrations/0006_media_pipeline.sql");
const LIBRARY_MIGRATION: &str = include_str!("../migrations/0007_library.sql");
const CARD_LIFECYCLE_MIGRATION: &str = include_str!("../migrations/0008_card_lifecycle.sql");
const PROJECTION_INTEGRITY_MIGRATION: &str =
    include_str!("../migrations/0009_projection_integrity.sql");
const TIME_BUDGET_POLICY_MIGRATION: &str =
    include_str!("../migrations/0010_time_budget_policy.sql");
const LATEST_SCHEMA_VERSION: u32 = 10;

pub const DEFAULT_DECK_ID: &str = "default-deck";
pub const DEFAULT_SCHEDULER_PARAMETER_SET_ID: &str = "fsrs7-default-v1";
#[cfg(any(test, feature = "test-fixtures"))]
pub const SAMPLE_SOURCE_ID: &str = "sample-source";
#[cfg(any(test, feature = "test-fixtures"))]
pub const SAMPLE_CLOZE_ID: &str = "sample-cloze";
#[cfg(any(test, feature = "test-fixtures"))]
pub const SAMPLE_CARD_ID: &str = "sample-card";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("stored JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("card {0} does not exist")]
    CardNotFound(String),
    #[error("{entity} {id} does not exist")]
    EntityNotFound { entity: &'static str, id: String },
    #[error("the card changed before the review could be committed")]
    StaleReview,
    #[error("card {0} has no latest review to undo")]
    NothingToUndo(String),
    #[error("media reference {id} is still used by {references} owner(s)")]
    MediaInUse { id: String, references: u64 },
    #[error("review history cannot rebuild the schedule projection: {0}")]
    ProjectionMismatch(String),
    #[error("invalid stored value for {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("invalid domain aggregate: {0}")]
    InvalidAggregate(String),
    #[error("database schema version {found} is newer than supported version {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("backup destination already exists: {}", .0.display())]
    BackupDestinationExists(std::path::PathBuf),
    #[error("storage filesystem operation {operation} failed for {}: {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("numeric value for {0} is outside the supported range")]
    NumericRange(&'static str),
    #[cfg(any(test, feature = "test-fixtures"))]
    #[error("injected local test failure at {0}")]
    InjectedTestFailure(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSourceNote {
    pub source_item: SourceItem,
    pub clozes: Vec<Cloze>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredStudyCard {
    pub source_item: SourceItem,
    pub cloze: Cloze,
    pub card: Card,
    pub schedule: ScheduleState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredLibraryCard {
    pub card: Card,
    pub schedule: ScheduleState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredLibraryNote {
    pub note: StoredSourceNote,
    pub cards: Vec<StoredLibraryCard>,
    pub deleted_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PristineDeckCard {
    pub card: Card,
    pub initial_schedule: ScheduleState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PristineDeckNote {
    pub note: StoredSourceNote,
    pub cards: Vec<PristineDeckCard>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PristineDeckImport {
    pub deck: Deck,
    pub notes: Vec<PristineDeckNote>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PristineDeckImportStatus {
    Ready,
    AlreadyInstalled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulingWorkload {
    pub unseen_cards: u64,
    pub due_cards_now: u64,
    pub forecast_review_occurrences: u64,
    pub response_duration_samples: u64,
    pub median_response_duration_ms: Option<u64>,
    pub review_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduleIntegrityReport {
    pub checked_cards: usize,
    pub mismatched_card_ids: Vec<String>,
}

impl ScheduleIntegrityReport {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.mismatched_card_ids.is_empty()
    }
}

pub struct Storage {
    pub(crate) connection: Connection,
}

impl Storage {
    /// Reports whether the collection contains any user learning material.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the collection cannot be queried.
    pub fn has_learning_material(&self) -> Result<bool, StorageError> {
        self.connection
            .query_row("SELECT EXISTS(SELECT 1 FROM source_items)", [], |row| {
                row.get(0)
            })
            .map_err(StorageError::from)
    }

    /// Opens or creates a collection and applies pending migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the database cannot be opened, configured,
    /// or migrated.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let connection = Connection::open(path)?;
        let mut storage = Self { connection };
        storage.configure()?;
        let current = storage.current_schema_version()?;
        if current > 0 && current < LATEST_SCHEMA_VERSION {
            storage.create_rolling_backup(path, &format!("migration-v{current}"), 5)?;
        }
        storage.migrate()?;
        Ok(storage)
    }

    /// Opens an isolated in-memory collection for tests and ephemeral use.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the database cannot be configured or
    /// migrated.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory()?;
        let mut storage = Self { connection };
        storage.configure()?;
        storage.migrate()?;
        Ok(storage)
    }

    fn configure(&self) -> Result<(), StorageError> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;",
        )?;
        Ok(())
    }

    fn migrate(&mut self) -> Result<(), StorageError> {
        let current = self.current_schema_version()?;

        if current > LATEST_SCHEMA_VERSION {
            return Err(StorageError::UnsupportedSchema {
                found: current,
                supported: LATEST_SCHEMA_VERSION,
            });
        }
        if current == LATEST_SCHEMA_VERSION {
            return Ok(());
        }

        let transaction = self.connection.transaction()?;
        if current < 1 {
            transaction.execute_batch(FOUNDATION_MIGRATION)?;
        }
        if current < 2 {
            transaction.execute_batch(CORE_MODEL_MIGRATION)?;
        }
        if current < 3 {
            transaction.execute_batch(AUTHORING_DEFAULTS_MIGRATION)?;
        }
        if current < 4 {
            transaction.execute_batch(FSRS7_SCHEDULER_MIGRATION)?;
        }
        if current < 5 {
            transaction.execute_batch(STUDY_SESSION_MIGRATION)?;
        }
        if current < 6 {
            transaction.execute_batch(MEDIA_PIPELINE_MIGRATION)?;
        }
        if current < 7 {
            transaction.execute_batch(LIBRARY_MIGRATION)?;
        }
        if current < 8 {
            transaction.execute_batch(CARD_LIFECYCLE_MIGRATION)?;
        }
        if current < 9 {
            let repaired_cards = review::repair_all_schedule_projections(&transaction)?;
            transaction.execute_batch(PROJECTION_INTEGRITY_MIGRATION)?;
            transaction.execute(
                "INSERT INTO schedule_projection_repairs(
                    schema_version, repaired_cards, repaired_at_ms
                 ) VALUES (9, ?1, unixepoch('subsec') * 1000)",
                [u64::try_from(repaired_cards)
                    .map_err(|_| StorageError::NumericRange("repaired card count"))?],
            )?;
        }
        if current < 10 {
            transaction.execute_batch(TIME_BUDGET_POLICY_MIGRATION)?;
        }
        transaction.commit()?;
        Ok(())
    }

    fn current_schema_version(&self) -> Result<u32, StorageError> {
        let has_schema_table = self
            .connection
            .query_row(
                "SELECT 1
                 FROM sqlite_schema
                 WHERE type = 'table' AND name = 'schema_migrations'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if has_schema_table {
            Ok(self.connection.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get::<_, u32>(0),
            )?)
        } else {
            Ok(0)
        }
    }

    /// Returns the latest applied schema version.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the migration table cannot be queried.
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        Ok(self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    /// Returns the number of projections repaired by the integrity migration.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the repair record cannot be queried.
    pub fn projection_migration_repaired_cards(&self) -> Result<u64, StorageError> {
        Ok(self.connection.query_row(
            "SELECT repaired_cards
             FROM schedule_projection_repairs
             WHERE schema_version = 9",
            [],
            |row| row.get(0),
        )?)
    }

    /// Creates a consistent backup of the open collection.
    ///
    /// The destination must not exist so callers cannot silently overwrite a
    /// previous recovery point.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the destination exists or `SQLite` cannot
    /// complete the backup.
    pub fn backup_to(&self, destination: &Path) -> Result<(), StorageError> {
        if destination.exists() {
            return Err(StorageError::BackupDestinationExists(
                destination.to_path_buf(),
            ));
        }
        self.connection.backup(MAIN_DB, destination, None)?;
        Ok(())
    }

    /// Creates and prunes a timestamped recovery backup beside a collection.
    ///
    /// The policy keeps the newest `keep` backups for the supplied reason.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the reason or retention count is invalid,
    /// a directory operation fails, or `SQLite` cannot create the backup.
    pub fn create_rolling_backup(
        &self,
        collection_path: &Path,
        reason: &str,
        keep: usize,
    ) -> Result<PathBuf, StorageError> {
        if keep == 0
            || reason.is_empty()
            || !reason
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(StorageError::InvalidAggregate(
                "rolling backup policy is invalid".into(),
            ));
        }
        let parent = collection_path.parent().unwrap_or_else(|| Path::new("."));
        let directory = parent.join("backups");
        fs::create_dir_all(&directory)
            .map_err(|error| storage_io("create backup directory", &directory, error))?;
        let name = collection_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("collection.db");
        let timestamp = self.connection.query_row(
            "SELECT CAST(unixepoch('subsec') * 1000 AS INTEGER)",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let destination = (0_u16..=u16::MAX)
            .map(|sequence| {
                directory.join(format!("{name}.{reason}-{timestamp}-{sequence:04}.bak"))
            })
            .find(|candidate| !candidate.exists())
            .ok_or_else(|| {
                StorageError::InvalidAggregate("no rolling backup filename is available".into())
            })?;
        self.backup_to(&destination)?;
        prune_rolling_backups(&directory, &format!("{name}.{reason}-"), keep)?;
        Ok(destination)
    }

    /// Restores a backup into a new collection path and applies pending
    /// migrations.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the destination already exists, the
    /// backup cannot be restored, or migrations fail.
    pub fn restore_from_backup(backup: &Path, destination: &Path) -> Result<Self, StorageError> {
        if destination.exists() {
            return Err(StorageError::BackupDestinationExists(
                destination.to_path_buf(),
            ));
        }
        let mut connection = Connection::open(destination)?;
        connection.restore(MAIN_DB, backup, None::<fn(rusqlite::backup::Progress)>)?;
        let mut storage = Self { connection };
        storage.configure()?;
        storage.migrate()?;
        Ok(storage)
    }

    /// Replaces a collection with a validated `SQLite` backup.
    ///
    /// Callers are responsible for creating a recovery backup before using
    /// this operation.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the backup fails integrity checks or the
    /// restore/migration cannot complete.
    pub fn replace_from_backup(backup: &Path, destination: &Path) -> Result<Self, StorageError> {
        let source =
            Connection::open_with_flags(backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let integrity =
            source.query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))?;
        if integrity != "ok" {
            return Err(StorageError::InvalidAggregate(format!(
                "backup integrity check failed: {integrity}"
            )));
        }
        drop(source);

        let mut connection = Connection::open(destination)?;
        connection.restore(MAIN_DB, backup, None::<fn(rusqlite::backup::Progress)>)?;
        let mut storage = Self { connection };
        storage.configure()?;
        storage.migrate()?;
        Ok(storage)
    }

    /// Inserts the stable sample source, cloze, card, and initial schedule.
    ///
    /// Existing sample rows are preserved.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when any insert or the surrounding transaction
    /// fails.
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn seed_walking_skeleton(&mut self, now_ms: i64) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT OR IGNORE INTO source_items(
                id, language_tag, direction, created_at_ms, deck_id, updated_at_ms
             ) VALUES (?1, 'ja', 'auto', ?2, ?3, ?2)",
            params![SAMPLE_SOURCE_ID, now_ms, DEFAULT_DECK_ID],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO clozes(
                id, source_item_id, answer, accepted_answers_json,
                language_tag, direction, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, '行きます', '[\"ゆきます\"]', 'ja', 'auto', ?3, ?3)",
            params![SAMPLE_CLOZE_ID, SAMPLE_SOURCE_ID, now_ms],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO semantic_segments(
                id, source_item_id, ordinal, kind, text, cloze_id
             ) VALUES ('sample-segment-context', ?1, 0, 'text', '日曜日は図書館に', NULL)",
            [SAMPLE_SOURCE_ID],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO semantic_segments(
                id, source_item_id, ordinal, kind, text, cloze_id
             ) VALUES ('sample-segment-cloze', ?1, 1, 'cloze', '行きます', ?2)",
            params![SAMPLE_SOURCE_ID, SAMPLE_CLOZE_ID],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO cards(
                id, cloze_id, content_version, created_at_ms, updated_at_ms,
                queue_updated_at_ms
             ) VALUES (?1, ?2, 0, ?3, ?3, ?3)",
            params![SAMPLE_CARD_ID, SAMPLE_CLOZE_ID, now_ms],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO schedule_states(
                card_id, version, lifecycle, due_at_ms, ideal_due_at_ms,
                interval_milliseconds, interval_seconds, repetitions,
                stability_milliseconds, difficulty_millipoints,
                last_reviewed_at_ms, last_review_event_id
             ) VALUES (?1, 0, 'unseen', ?2, ?2, 0, 0, 0, 0, 0, NULL, NULL)",
            params![SAMPLE_CARD_ID, now_ms],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO schedule_baselines(
                card_id, version, lifecycle, due_at_ms, ideal_due_at_ms,
                interval_milliseconds, interval_seconds, repetitions,
                stability_milliseconds, difficulty_millipoints,
                last_reviewed_at_ms, last_review_event_id
             ) VALUES (?1, 0, 'unseen', ?2, ?2, 0, 0, 0, 0, 0, NULL, NULL)",
            params![SAMPLE_CARD_ID, now_ms],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Inserts a deterministic, storage-backed mixed-script release fixture.
    ///
    /// This helper is available only to tests and explicit test-fixture builds.
    /// It uses production schema constraints and creates valid review history
    /// for introduced cards.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the fixture cannot be committed.
    #[cfg(any(test, feature = "test-fixtures"))]
    #[allow(clippy::too_many_lines)]
    pub fn seed_large_performance_fixture(
        &mut self,
        card_count: u32,
        now_ms: i64,
    ) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT OR IGNORE INTO decks(
                id, name, language_tag, direction, matching_policy,
                created_at_ms, updated_at_ms
             ) VALUES (
                'performance-deck', 'Performance 日本語 العربية', 'und',
                'auto', 'strict', ?1, ?1
             )",
            [now_ms],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO scheduler_profiles(
                deck_id, engine_version, active_parameter_set_id, updated_at_ms
             ) VALUES (
                'performance-deck', 'fsrs-7', ?1, ?2
             )",
            params![DEFAULT_SCHEDULER_PARAMETER_SET_ID, now_ms],
        )?;

        {
            let mut insert_source = transaction.prepare(
                "INSERT INTO source_items(
                    id, language_tag, direction, created_at_ms, deck_id, updated_at_ms
                 ) VALUES (?1, 'und', 'auto', ?2, ?3, ?2)",
            )?;
            let mut insert_cloze = transaction.prepare(
                "INSERT INTO clozes(
                    id, source_item_id, answer, accepted_answers_json,
                    language_tag, direction, created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, '[]', 'und', 'auto', ?4, ?4)",
            )?;
            let mut insert_segment = transaction.prepare(
                "INSERT INTO semantic_segments(
                    id, source_item_id, ordinal, kind, text, cloze_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut insert_card = transaction.prepare(
                "INSERT INTO cards(
                    id, cloze_id, content_version, created_at_ms,
                    updated_at_ms, queue_updated_at_ms
                 ) VALUES (?1, ?2, 0, ?3, ?3, ?3)",
            )?;
            let mut insert_schedule = transaction.prepare(
                "INSERT INTO schedule_states(
                    card_id, version, lifecycle, due_at_ms, ideal_due_at_ms,
                    interval_milliseconds, interval_seconds, repetitions,
                    stability_milliseconds, difficulty_millipoints,
                    last_reviewed_at_ms, last_review_event_id
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11
                 )",
            )?;
            let mut insert_baseline = transaction.prepare(
                "INSERT INTO schedule_baselines(
                    card_id, version, lifecycle, due_at_ms, ideal_due_at_ms,
                    interval_milliseconds, interval_seconds, repetitions,
                    stability_milliseconds, difficulty_millipoints,
                    last_reviewed_at_ms, last_review_event_id
                 ) VALUES (
                    ?1, 0, 'unseen', ?2, ?2, 0, 0, 0, 0, 0, NULL, NULL
                 )",
            )?;
            let mut insert_review = transaction.prepare(
                "INSERT INTO review_events(
                    id, card_id, card_content_version, raw_response,
                    normalized_response, comparison, suggested_grade,
                    chosen_grade, reviewed_at_ms, scheduler_version,
                    scheduler_parameter_set_id, target_retention_basis_points,
                    previous_schedule_version, previous_due_at_ms,
                    previous_ideal_due_at_ms, previous_interval_milliseconds,
                    previous_interval_seconds, previous_repetitions,
                    previous_stability_milliseconds,
                    previous_difficulty_millipoints,
                    previous_last_reviewed_at_ms, next_schedule_version,
                    next_due_at_ms, next_ideal_due_at_ms,
                    next_interval_milliseconds, next_interval_seconds,
                    next_repetitions, next_stability_milliseconds,
                    next_difficulty_millipoints, next_last_reviewed_at_ms,
                    event_kind, undoes_review_event_id, response_duration_ms,
                    grade_overridden, previous_lifecycle, next_lifecycle
                 ) VALUES (
                    ?1, ?2, 0, ?3, ?3, 'exact', 'good', 'good', ?4, 'fsrs-7',
                    ?5, 9000, 0, ?4, ?4, 0, 0, 0, 0, 0, NULL,
                    1, ?6, ?6, 86400000, 86400, 1, 86400000, 5000, ?4,
                    'review', NULL, ?7, 0, 'unseen', 'introduced'
                 )",
            )?;

            for index in 0..card_count {
                let suffix = format!("{index:07}");
                let source_id = format!("performance-source-{suffix}");
                let cloze_id = format!("performance-cloze-{suffix}");
                let card_id = format!("performance-card-{suffix}");
                let event_id = format!("performance-review-{suffix}");
                let deck_id = if index % 2 == 0 {
                    DEFAULT_DECK_ID
                } else {
                    "performance-deck"
                };
                let (context, answer) = match index % 5 {
                    0 => ("東京 العربية search-target", "図書館"),
                    1 => ("Cafe\u{301} आणि पुस्तक", "café"),
                    2 => ("Meeting الساعة 三時", "三時"),
                    3 => ("👩🏽‍💻 family 👨‍👩‍👧‍👦", "emoji"),
                    _ => ("עברית ١٢٣ punctuation", "שלום"),
                };
                insert_source.execute(params![source_id, now_ms, deck_id])?;
                insert_cloze.execute(params![cloze_id, source_id, answer, now_ms])?;
                insert_segment.execute(params![
                    format!("performance-context-{suffix}"),
                    source_id,
                    0,
                    "text",
                    context,
                    Option::<String>::None,
                ])?;
                insert_segment.execute(params![
                    format!("performance-answer-{suffix}"),
                    source_id,
                    1,
                    "cloze",
                    answer,
                    cloze_id,
                ])?;
                insert_card.execute(params![card_id, cloze_id, now_ms])?;

                if index % 3 == 2 {
                    let due_at_ms = now_ms.saturating_add(i64::from(index));
                    insert_baseline.execute(params![card_id, due_at_ms])?;
                    insert_schedule.execute(params![
                        card_id,
                        0,
                        "unseen",
                        due_at_ms,
                        0,
                        0,
                        0,
                        0,
                        0,
                        Option::<i64>::None,
                        Option::<String>::None,
                    ])?;
                } else {
                    let due_at_ms = if index % 3 == 0 {
                        now_ms
                    } else {
                        now_ms.saturating_sub(1)
                    };
                    let reviewed_at_ms = now_ms.saturating_sub(86_400_000);
                    insert_baseline.execute(params![card_id, reviewed_at_ms])?;
                    insert_review.execute(params![
                        event_id,
                        card_id,
                        answer,
                        reviewed_at_ms,
                        DEFAULT_SCHEDULER_PARAMETER_SET_ID,
                        due_at_ms,
                        1_000_u64 + u64::from(index % 2_000),
                    ])?;
                    insert_schedule.execute(params![
                        card_id,
                        1,
                        "introduced",
                        due_at_ms,
                        86_400_000,
                        86_400,
                        1,
                        86_400_000,
                        5_000,
                        reviewed_at_ms,
                        event_id,
                    ])?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

fn prune_rolling_backups(directory: &Path, prefix: &str, keep: usize) -> Result<(), StorageError> {
    let entries = fs::read_dir(directory)
        .map_err(|error| storage_io("read backup directory", directory, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| storage_io("read backup directory entry", directory, error))?;
    let mut backups = entries
        .into_iter()
        .filter(|entry| {
            entry.path().is_file()
                && entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with(prefix)
                        && Path::new(name)
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("bak"))
                })
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(std::fs::DirEntry::file_name);
    let remove_count = backups.len().saturating_sub(keep);
    for entry in backups.into_iter().take(remove_count) {
        let path = entry.path();
        fs::remove_file(&path).map_err(|error| storage_io("prune backup", &path, error))?;
    }
    Ok(())
}

fn storage_io(operation: &'static str, path: &Path, source: io::Error) -> StorageError {
    StorageError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

pub(crate) fn direction_from_database(value: &str) -> Result<Direction, StorageError> {
    match value {
        "auto" => Ok(Direction::Auto),
        "ltr" => Ok(Direction::LeftToRight),
        "rtl" => Ok(Direction::RightToLeft),
        _ => Err(StorageError::InvalidStoredValue {
            field: "direction",
            value: value.to_owned(),
        }),
    }
}

pub(crate) const fn direction_to_database(value: Direction) -> &'static str {
    match value {
        Direction::Auto => "auto",
        Direction::LeftToRight => "ltr",
        Direction::RightToLeft => "rtl",
    }
}

pub(crate) fn matching_policy_from_database(value: &str) -> Result<MatchingPolicy, StorageError> {
    match value {
        "strict" => Ok(MatchingPolicy::Strict),
        "forgiving" => Ok(MatchingPolicy::Forgiving),
        _ => Err(StorageError::InvalidStoredValue {
            field: "matching policy",
            value: value.to_owned(),
        }),
    }
}

pub(crate) const fn matching_policy_to_database(value: MatchingPolicy) -> &'static str {
    match value {
        MatchingPolicy::Strict => "strict",
        MatchingPolicy::Forgiving => "forgiving",
    }
}

pub(crate) fn entity_not_found(entity: &'static str, id: &str) -> StorageError {
    StorageError::EntityNotFound {
        entity,
        id: id.to_owned(),
    }
}

#[cfg(test)]
mod tests;
