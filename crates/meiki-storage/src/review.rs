use meiki_domain::{ComparisonResult, Grade, ReviewEvent, ScheduleState};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::{Storage, StorageError, repository::load_schedule_row};

impl Storage {
    /// Atomically appends a review event, advances the schedule projection, and
    /// updates queue-relevant card metadata.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::StaleReview`] when observed versions or
    /// snapshots are stale, or another [`StorageError`] when the transaction
    /// fails.
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
             WHERE card_id = ?6
               AND version = ?7
               AND due_at_ms = ?8
               AND interval_seconds = ?9
               AND repetitions = ?10
               AND last_review_event_id IS ?11",
            params![
                event.next_schedule.version,
                event.next_schedule.due_at_ms,
                event.next_schedule.interval_seconds,
                event.next_schedule.repetitions,
                event.id,
                event.card_id,
                event.previous_schedule.version,
                event.previous_schedule.due_at_ms,
                event.previous_schedule.interval_seconds,
                event.previous_schedule.repetitions,
                event.previous_schedule.last_review_event_id,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::StaleReview);
        }

        let changed = transaction.execute(
            "UPDATE cards
             SET queue_updated_at_ms = ?1
             WHERE id = ?2 AND content_version = ?3",
            params![
                event.reviewed_at_ms,
                event.card_id,
                event.card_content_version,
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

    /// Loads immutable review events in deterministic chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when stored values cannot be decoded.
    pub fn review_events(&self, card_id: &str) -> Result<Vec<ReviewEvent>, StorageError> {
        load_review_events(&self.connection, card_id)
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

    /// Rebuilds the current schedule projection from its baseline and immutable
    /// review events.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::ProjectionMismatch`] when the history is not a
    /// contiguous state transition chain.
    pub fn rebuild_schedule_projection(
        &mut self,
        card_id: &str,
    ) -> Result<ScheduleState, StorageError> {
        let transaction = self.connection.transaction()?;
        let mut projected = load_schedule_row(&transaction, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let events = load_review_events(&transaction, card_id)?;
        let mut queue_updated_at_ms = 0;

        for event in events {
            if event.previous_schedule != projected
                || event.next_schedule.card_id != card_id
                || event.next_schedule.version != projected.version + 1
            {
                return Err(StorageError::ProjectionMismatch(format!(
                    "event {} does not continue version {}",
                    event.id, projected.version
                )));
            }
            queue_updated_at_ms = event.reviewed_at_ms;
            projected = event.next_schedule;
        }

        let changed = transaction.execute(
            "UPDATE schedule_states
             SET version = ?1,
                 due_at_ms = ?2,
                 interval_seconds = ?3,
                 repetitions = ?4,
                 last_review_event_id = ?5
             WHERE card_id = ?6",
            params![
                projected.version,
                projected.due_at_ms,
                projected.interval_seconds,
                projected.repetitions,
                projected.last_review_event_id,
                card_id,
            ],
        )?;
        if changed != 1 {
            return Err(StorageError::CardNotFound(card_id.to_owned()));
        }
        transaction.execute(
            "UPDATE cards
             SET queue_updated_at_ms = ?1
             WHERE id = ?2",
            params![queue_updated_at_ms, card_id],
        )?;
        transaction.commit()?;
        Ok(projected)
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
    let stored_schedule = load_schedule_row(transaction, "schedule_states", &event.card_id)?
        .ok_or_else(|| StorageError::CardNotFound(event.card_id.clone()))?;

    if event.previous_schedule.card_id != event.card_id
        || event.next_schedule.card_id != event.card_id
        || event.next_schedule.last_review_event_id.as_deref() != Some(event.id.as_str())
        || card_version != event.card_content_version
        || stored_schedule != event.previous_schedule
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
            next_repetitions,
            scheduler_parameter_set_id
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19
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
            event.scheduler_parameter_set_id,
        ],
    )?;
    Ok(())
}

fn load_review_events(
    connection: &Connection,
    card_id: &str,
) -> Result<Vec<ReviewEvent>, StorageError> {
    let mut statement = connection.prepare(
        "SELECT
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
            next_repetitions,
            scheduler_parameter_set_id
         FROM review_events
         WHERE card_id = ?1
         ORDER BY previous_schedule_version, reviewed_at_ms, id",
    )?;
    let stored = statement
        .query_map([card_id], stored_review_event_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let baseline = load_schedule_row(connection, "schedule_baselines", card_id)?
        .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
    let mut previous_event_id = baseline.last_review_event_id;
    let mut events = Vec::with_capacity(stored.len());
    for stored in stored {
        let event = stored.into_domain(previous_event_id)?;
        previous_event_id = Some(event.id.clone());
        events.push(event);
    }
    Ok(events)
}

struct StoredReviewEvent {
    id: String,
    card_id: String,
    card_content_version: u64,
    raw_response: String,
    normalized_response: String,
    comparison: String,
    suggested_grade: String,
    chosen_grade: String,
    reviewed_at_ms: i64,
    scheduler_version: String,
    previous_version: u64,
    previous_due_at_ms: i64,
    previous_interval_seconds: u64,
    previous_repetitions: u32,
    next_version: u64,
    next_due_at_ms: i64,
    next_interval_seconds: u64,
    next_repetitions: u32,
    scheduler_parameter_set_id: Option<String>,
}

impl StoredReviewEvent {
    fn into_domain(self, previous_event_id: Option<String>) -> Result<ReviewEvent, StorageError> {
        let previous_schedule = ScheduleState {
            card_id: self.card_id.clone(),
            version: self.previous_version,
            due_at_ms: self.previous_due_at_ms,
            interval_seconds: self.previous_interval_seconds,
            repetitions: self.previous_repetitions,
            last_review_event_id: previous_event_id,
        };
        let next_schedule = ScheduleState {
            card_id: self.card_id.clone(),
            version: self.next_version,
            due_at_ms: self.next_due_at_ms,
            interval_seconds: self.next_interval_seconds,
            repetitions: self.next_repetitions,
            last_review_event_id: Some(self.id.clone()),
        };
        Ok(ReviewEvent {
            id: self.id,
            card_id: self.card_id,
            card_content_version: self.card_content_version,
            raw_response: self.raw_response,
            normalized_response: self.normalized_response,
            comparison: comparison_from_database(&self.comparison)?,
            suggested_grade: grade_from_database(&self.suggested_grade)?,
            chosen_grade: grade_from_database(&self.chosen_grade)?,
            reviewed_at_ms: self.reviewed_at_ms,
            scheduler_version: self.scheduler_version,
            scheduler_parameter_set_id: self.scheduler_parameter_set_id,
            previous_schedule,
            next_schedule,
        })
    }
}

fn stored_review_event_from_row(row: &Row<'_>) -> rusqlite::Result<StoredReviewEvent> {
    Ok(StoredReviewEvent {
        id: row.get(0)?,
        card_id: row.get(1)?,
        card_content_version: row.get(2)?,
        raw_response: row.get(3)?,
        normalized_response: row.get(4)?,
        comparison: row.get(5)?,
        suggested_grade: row.get(6)?,
        chosen_grade: row.get(7)?,
        reviewed_at_ms: row.get(8)?,
        scheduler_version: row.get(9)?,
        previous_version: row.get(10)?,
        previous_due_at_ms: row.get(11)?,
        previous_interval_seconds: row.get(12)?,
        previous_repetitions: row.get(13)?,
        next_version: row.get(14)?,
        next_due_at_ms: row.get(15)?,
        next_interval_seconds: row.get(16)?,
        next_repetitions: row.get(17)?,
        scheduler_parameter_set_id: row.get(18)?,
    })
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

fn comparison_from_database(value: &str) -> Result<ComparisonResult, StorageError> {
    match value {
        "exact" => Ok(ComparisonResult::Exact),
        "accepted_variant" => Ok(ComparisonResult::AcceptedVariant),
        "near_match" => Ok(ComparisonResult::NearMatch),
        "incorrect" => Ok(ComparisonResult::Incorrect),
        "empty" => Ok(ComparisonResult::Empty),
        _ => Err(StorageError::InvalidStoredValue {
            field: "comparison",
            value: value.to_owned(),
        }),
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

fn grade_from_database(value: &str) -> Result<Grade, StorageError> {
    match value {
        "again" => Ok(Grade::Again),
        "hard" => Ok(Grade::Hard),
        "good" => Ok(Grade::Good),
        "easy" => Ok(Grade::Easy),
        _ => Err(StorageError::InvalidStoredValue {
            field: "grade",
            value: value.to_owned(),
        }),
    }
}
