//! `SQLite` implementation of Meiki persistence.
//!
//! SQL is owned by this crate and must not leak through its public interface.

use std::path::Path;

use meiki_domain::{
    Card, Cloze, ComparisonResult, Direction, Grade, ReviewEvent, ScheduleState, SegmentContent,
    SemanticSegment, SourceItem,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;

const FOUNDATION_MIGRATION: &str = include_str!("../migrations/0001_foundation.sql");

pub const SAMPLE_SOURCE_ID: &str = "sample-source";
pub const SAMPLE_CLOZE_ID: &str = "sample-cloze";
pub const SAMPLE_CARD_ID: &str = "sample-card";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("storage operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("stored accepted answers are invalid: {0}")]
    InvalidAcceptedAnswers(#[from] serde_json::Error),
    #[error("card {0} does not exist")]
    CardNotFound(String),
    #[error("the card changed before the review could be committed")]
    StaleReview,
    #[error("invalid stored value for {field}: {value}")]
    InvalidStoredValue { field: &'static str, value: String },
    #[error("numeric value for {0} is outside the supported range")]
    NumericRange(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredStudyCard {
    pub source_item: SourceItem,
    pub cloze: Cloze,
    pub card: Card,
    pub schedule: ScheduleState,
}

pub struct Storage {
    connection: Connection,
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

        if !has_schema_table {
            self.connection.execute_batch(FOUNDATION_MIGRATION)?;
        }
        Ok(())
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
                id, language_tag, direction, created_at_ms
             ) VALUES (?1, 'ja', 'auto', ?2)",
            params![SAMPLE_SOURCE_ID, now_ms],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO clozes(
                id, source_item_id, answer, accepted_answers_json
             ) VALUES (?1, ?2, '行きます', '[\"ゆきます\"]')",
            params![SAMPLE_CLOZE_ID, SAMPLE_SOURCE_ID],
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
            "INSERT OR IGNORE INTO cards(id, cloze_id, content_version)
             VALUES (?1, ?2, 0)",
            params![SAMPLE_CARD_ID, SAMPLE_CLOZE_ID],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO schedule_states(
                card_id, version, due_at_ms, interval_seconds, repetitions, last_review_event_id
             ) VALUES (?1, 0, ?2, 0, 0, NULL)",
            params![SAMPLE_CARD_ID, now_ms],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads a card with its source segments, cloze, and projected schedule.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the card is missing or stored data cannot
    /// be decoded.
    pub fn load_study_card(&self, card_id: &str) -> Result<StoredStudyCard, StorageError> {
        let (card, cloze, source_id, language_tag, direction) = self
            .connection
            .query_row(
                "SELECT
                    cards.id,
                    cards.cloze_id,
                    cards.content_version,
                    clozes.source_item_id,
                    clozes.answer,
                    clozes.accepted_answers_json,
                    source_items.language_tag,
                    source_items.direction
                 FROM cards
                 JOIN clozes ON clozes.id = cards.cloze_id
                 JOIN source_items ON source_items.id = clozes.source_item_id
                 WHERE cards.id = ?1",
                [card_id],
                |row| {
                    Ok((
                        Card {
                            id: row.get(0)?,
                            cloze_id: row.get(1)?,
                            content_version: row.get(2)?,
                        },
                        (
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                        ),
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;

        let accepted_answers = serde_json::from_str(&cloze.2)?;
        let cloze = Cloze {
            id: card.cloze_id.clone(),
            source_item_id: cloze.0,
            answer: cloze.1,
            accepted_answers,
        };

        let mut statement = self.connection.prepare(
            "SELECT id, ordinal, kind, text, cloze_id
             FROM semantic_segments
             WHERE source_item_id = ?1
             ORDER BY ordinal",
        )?;
        let segments = statement
            .query_map([&source_id], |row| {
                let ordinal = row.get::<_, u32>(1)?;
                let kind = row.get::<_, String>(2)?;
                let text = row.get::<_, String>(3)?;
                let content = match kind.as_str() {
                    "text" => SegmentContent::Text(text),
                    "cloze" => SegmentContent::Cloze {
                        cloze_id: row.get::<_, String>(4)?,
                        text,
                    },
                    _ => {
                        return Err(rusqlite::Error::InvalidColumnType(
                            2,
                            "kind".to_owned(),
                            rusqlite::types::Type::Text,
                        ));
                    }
                };
                Ok(SemanticSegment {
                    id: row.get(0)?,
                    ordinal,
                    content,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let schedule = self.load_schedule(card_id)?;
        Ok(StoredStudyCard {
            source_item: SourceItem {
                id: source_id,
                segments,
                language_tag,
                direction: direction_from_database(&direction)?,
            },
            cloze,
            card,
            schedule,
        })
    }

    /// Loads the current projected schedule for a card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the card is missing or the query fails.
    pub fn load_schedule(&self, card_id: &str) -> Result<ScheduleState, StorageError> {
        self.connection
            .query_row(
                "SELECT
                    card_id, version, due_at_ms, interval_seconds, repetitions,
                    last_review_event_id
                 FROM schedule_states
                 WHERE card_id = ?1",
                [card_id],
                |row| {
                    Ok(ScheduleState {
                        card_id: row.get(0)?,
                        version: row.get(1)?,
                        due_at_ms: row.get(2)?,
                        interval_seconds: row.get(3)?,
                        repetitions: row.get(4)?,
                        last_review_event_id: row.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))
    }

    /// Atomically appends a review event and advances the schedule projection.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::StaleReview`] when observed versions are stale,
    /// or another [`StorageError`] when the transaction fails.
    pub fn commit_review(&mut self, event: &ReviewEvent) -> Result<ScheduleState, StorageError> {
        let transaction = self.connection.transaction()?;
        validate_review_preconditions(&transaction, event)?;
        insert_review_event(&transaction, event)?;

        let changed = transaction.execute(
            "UPDATE schedule_states
             SET version = ?1,
                 due_at_ms = ?2,
                 interval_seconds = ?3,
                 repetitions = ?4,
                 last_review_event_id = ?5
             WHERE card_id = ?6 AND version = ?7",
            params![
                event.next_schedule.version,
                event.next_schedule.due_at_ms,
                event.next_schedule.interval_seconds,
                event.next_schedule.repetitions,
                event.id,
                event.card_id,
                event.previous_schedule.version,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::StaleReview);
        }

        transaction.commit()?;
        let mut committed = event.next_schedule.clone();
        committed.last_review_event_id = Some(event.id.clone());
        Ok(committed)
    }

    /// Counts immutable review events for a card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the query fails.
    pub fn review_count(&self, card_id: &str) -> Result<u64, StorageError> {
        let count = self.connection.query_row(
            "SELECT COUNT(*) FROM review_events WHERE card_id = ?1",
            [card_id],
            |row| row.get::<_, u64>(0),
        )?;
        Ok(count)
    }
}

fn validate_review_preconditions(
    transaction: &Transaction<'_>,
    event: &ReviewEvent,
) -> Result<(), StorageError> {
    let card_version = transaction
        .query_row(
            "SELECT content_version FROM cards WHERE id = ?1",
            [&event.card_id],
            |row| row.get::<_, u64>(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::CardNotFound(event.card_id.clone()))?;
    let schedule_version = transaction.query_row(
        "SELECT version FROM schedule_states WHERE card_id = ?1",
        [&event.card_id],
        |row| row.get::<_, u64>(0),
    )?;

    if card_version != event.card_content_version
        || schedule_version != event.previous_schedule.version
        || event.next_schedule.version != event.previous_schedule.version + 1
    {
        return Err(StorageError::StaleReview);
    }
    Ok(())
}

fn insert_review_event(
    transaction: &Transaction<'_>,
    event: &ReviewEvent,
) -> Result<(), StorageError> {
    transaction.execute(
        "INSERT INTO review_events(
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
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
            ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18
         )",
        params![
            event.id,
            event.card_id,
            event.card_content_version,
            event.raw_response,
            event.normalized_response,
            comparison_to_database(event.comparison),
            grade_to_database(event.suggested_grade),
            grade_to_database(event.chosen_grade),
            event.reviewed_at_ms,
            event.scheduler_version,
            event.previous_schedule.version,
            event.previous_schedule.due_at_ms,
            event.previous_schedule.interval_seconds,
            event.previous_schedule.repetitions,
            event.next_schedule.version,
            event.next_schedule.due_at_ms,
            event.next_schedule.interval_seconds,
            event.next_schedule.repetitions,
        ],
    )?;
    Ok(())
}

fn direction_from_database(value: &str) -> Result<Direction, StorageError> {
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

const fn comparison_to_database(value: ComparisonResult) -> &'static str {
    match value {
        ComparisonResult::Exact => "exact",
        ComparisonResult::AcceptedVariant => "accepted_variant",
        ComparisonResult::NearMatch => "near_match",
        ComparisonResult::Incorrect => "incorrect",
        ComparisonResult::Empty => "empty",
    }
}

const fn grade_to_database(value: Grade) -> &'static str {
    match value {
        Grade::Again => "again",
        Grade::Hard => "hard",
        Grade::Good => "good",
        Grade::Easy => "easy",
    }
}

#[cfg(test)]
mod tests {
    use meiki_domain::{ComparisonResult, Grade, ReviewEvent};
    use tempfile::tempdir;

    use super::{SAMPLE_CARD_ID, Storage, StorageError};

    fn sample_event(storage: &Storage, id: &str) -> ReviewEvent {
        let stored = storage.load_study_card(SAMPLE_CARD_ID).unwrap();
        let mut next = stored.schedule.clone();
        next.version += 1;
        next.due_at_ms = 259_210_000;
        next.interval_seconds = 259_200;
        next.repetitions += 1;
        next.last_review_event_id = Some(id.to_owned());
        ReviewEvent {
            id: id.to_owned(),
            card_id: stored.card.id,
            card_content_version: stored.card.content_version,
            raw_response: "行きます".into(),
            normalized_response: "行きます".into(),
            comparison: ComparisonResult::Exact,
            suggested_grade: Grade::Good,
            chosen_grade: Grade::Good,
            reviewed_at_ms: 10_000,
            scheduler_version: "test-scheduler".into(),
            previous_schedule: stored.schedule,
            next_schedule: next,
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
    }

    #[test]
    fn review_append_and_projection_update_are_atomic() {
        let mut storage = Storage::open_in_memory().unwrap();
        storage.seed_walking_skeleton(1_000).unwrap();
        let event = sample_event(&storage, "review-1");

        let committed = storage.commit_review(&event).unwrap();
        assert_eq!(committed.version, 1);
        assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);

        let stale = sample_event(&storage, "review-stale");
        let mut stale = stale;
        stale.previous_schedule.version = 0;
        assert!(matches!(
            storage.commit_review(&stale),
            Err(StorageError::StaleReview)
        ));
        assert_eq!(storage.review_count(SAMPLE_CARD_ID).unwrap(), 1);
    }

    #[test]
    fn review_events_cannot_be_changed_in_place() {
        let mut storage = Storage::open_in_memory().unwrap();
        storage.seed_walking_skeleton(1_000).unwrap();
        let event = sample_event(&storage, "review-1");
        storage.commit_review(&event).unwrap();

        let error = storage
            .connection
            .execute(
                "UPDATE review_events SET raw_response = 'changed' WHERE id = 'review-1'",
                [],
            )
            .unwrap_err();
        assert!(error.to_string().contains("append-only"));
    }
}
