//! `SQLite` implementation of Meiki persistence.
//!
//! SQL is owned by this crate and must not leak through its public interface.

mod repository;
mod review;

pub use repository::{
    AnnotationRepository, CardRepository, ClozeRepository, DeckRepository, MediaRepository,
    SchedulerParameterSetRepository, SourceNoteRepository, TagRepository,
};

use std::path::Path;

use meiki_domain::{Card, Cloze, Direction, MatchingPolicy, ScheduleState, SourceItem};
use rusqlite::{Connection, MAIN_DB, OptionalExtension, params};
use thiserror::Error;

const FOUNDATION_MIGRATION: &str = include_str!("../migrations/0001_foundation.sql");
const CORE_MODEL_MIGRATION: &str = include_str!("../migrations/0002_core_model.sql");
const AUTHORING_DEFAULTS_MIGRATION: &str =
    include_str!("../migrations/0003_authoring_defaults.sql");
const LATEST_SCHEMA_VERSION: u32 = 3;

pub const DEFAULT_DECK_ID: &str = "default-deck";
pub const SAMPLE_SOURCE_ID: &str = "sample-source";
pub const SAMPLE_CLOZE_ID: &str = "sample-cloze";
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
    #[error("numeric value for {0} is outside the supported range")]
    NumericRange(&'static str),
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

pub struct Storage {
    pub(crate) connection: Connection,
}

impl Storage {
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
        let current = if has_schema_table {
            self.connection.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get::<_, u32>(0),
            )?
        } else {
            0
        };

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
        transaction.commit()?;
        Ok(())
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

    /// Creates a consistent online backup of the open collection.
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

    /// Inserts the stable sample source, cloze, card, and initial schedule.
    ///
    /// Existing sample rows are preserved.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when any insert or the surrounding transaction
    /// fails.
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
                card_id, version, due_at_ms, interval_seconds, repetitions,
                last_review_event_id
             ) VALUES (?1, 0, ?2, 0, 0, NULL)",
            params![SAMPLE_CARD_ID, now_ms],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO schedule_baselines(
                card_id, version, due_at_ms, interval_seconds, repetitions,
                last_review_event_id
             ) VALUES (?1, 0, ?2, 0, 0, NULL)",
            params![SAMPLE_CARD_ID, now_ms],
        )?;
        transaction.commit()?;
        Ok(())
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
