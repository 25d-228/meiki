use meiki_domain::{
    CardLifecycle, ComparisonResult, Grade, ReviewEvent, ReviewEventKind, ScheduleState,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::{
    ScheduleIntegrityReport, Storage, StorageError,
    repository::{card_lifecycle_from_database, card_lifecycle_to_database, load_schedule_row},
};

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
        persist_review_event(&transaction, event)?;
        transaction.commit()?;
        let mut committed = event.next_schedule.clone();
        committed.last_review_event_id = Some(event.id.clone());
        Ok(committed)
    }

    /// Exercises rollback after all review writes but before transaction commit.
    ///
    /// This bounded fault is available only to local tests and fixture builds.
    ///
    /// # Errors
    ///
    /// Always returns [`StorageError::InjectedTestFailure`] after issuing the
    /// same writes as [`Self::commit_review`] inside an uncommitted transaction.
    #[cfg(any(test, feature = "test-fixtures"))]
    pub fn commit_review_failing_before_commit(
        &mut self,
        event: &ReviewEvent,
    ) -> Result<ScheduleState, StorageError> {
        let transaction = self.connection.transaction()?;
        persist_review_event(&transaction, event)?;
        Err(StorageError::InjectedTestFailure(
            "review transaction before commit",
        ))
    }

    /// Appends a compensating event for the latest review and restores the
    /// schedule values that existed immediately before that review.
    ///
    /// The restored projection receives a new monotonically increasing
    /// version, so stale callers cannot commit against the pre-undo snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::NothingToUndo`] unless the current projection
    /// points at an active review event.
    pub fn undo_last_review(
        &mut self,
        card_id: &str,
        expected_review_event_id: &str,
        undo_event_id: &str,
        undone_at_ms: i64,
    ) -> Result<ScheduleState, StorageError> {
        let transaction = self.connection.transaction()?;
        let current = load_schedule_row(&transaction, "schedule_states", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let (_, events, expected, _) = load_recorded_projection(&transaction, card_id)?;
        if current != expected {
            return Err(StorageError::ProjectionMismatch(format!(
                "card {card_id} current projection does not match immutable history"
            )));
        }
        let latest_id = expected
            .last_review_event_id
            .as_deref()
            .ok_or_else(|| StorageError::NothingToUndo(card_id.to_owned()))?;
        let target = events
            .into_iter()
            .find(|event| event.id == latest_id && event.kind == ReviewEventKind::Review)
            .ok_or_else(|| StorageError::NothingToUndo(card_id.to_owned()))?;
        if target.id != expected_review_event_id {
            return Err(StorageError::StaleReview);
        }

        let card_content_version = transaction.query_row(
            "SELECT content_version FROM cards WHERE id = ?1",
            [card_id],
            |row| row.get::<_, u64>(0),
        )?;
        let mut restored = target.previous_schedule.clone();
        restored.version = current.version + 1;
        restored.last_review_event_id = Some(undo_event_id.to_owned());
        let event = ReviewEvent {
            id: undo_event_id.to_owned(),
            card_id: card_id.to_owned(),
            card_content_version,
            kind: ReviewEventKind::Undo,
            undoes_review_event_id: Some(target.id),
            raw_response: String::new(),
            normalized_response: String::new(),
            comparison: target.comparison,
            suggested_grade: target.suggested_grade,
            chosen_grade: target.chosen_grade,
            grade_overridden: false,
            response_duration_ms: 0,
            reviewed_at_ms: undone_at_ms,
            scheduler_version: target.scheduler_version,
            scheduler_parameter_set_id: target.scheduler_parameter_set_id,
            target_retention_basis_points: target.target_retention_basis_points,
            previous_schedule: current,
            next_schedule: restored.clone(),
        };
        persist_review_event(&transaction, &event)?;
        transaction.commit()?;
        Ok(restored)
    }

    /// Loads immutable review events in deterministic chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when stored values cannot be decoded.
    pub fn review_events(&self, card_id: &str) -> Result<Vec<ReviewEvent>, StorageError> {
        load_review_events(&self.connection, card_id)
    }

    /// Loads graded reviews that have not been compensated by an undo event.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when stored values cannot be decoded or an
    /// invalid compensating chain is encountered.
    pub fn active_review_events(&self, card_id: &str) -> Result<Vec<ReviewEvent>, StorageError> {
        active_reviews(load_review_events(&self.connection, card_id)?)
    }

    /// Counts active graded reviews for a card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the query fails.
    pub fn review_count(&self, card_id: &str) -> Result<u64, StorageError> {
        u64::try_from(self.active_review_events(card_id)?.len())
            .map_err(|_| StorageError::NumericRange("review count"))
    }

    /// Loads the immutable schedule baseline used to rebuild a card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the card or baseline does not exist.
    pub fn load_schedule_baseline(&self, card_id: &str) -> Result<ScheduleState, StorageError> {
        load_schedule_row(&self.connection, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))
    }

    /// Restores validated immutable history and its current projection.
    ///
    /// This operation is intended for a staging collection during portable
    /// import. The card must have no existing events and its current schedule
    /// must still equal the supplied baseline.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the history chain is inconsistent, the
    /// card is not pristine, or the transaction cannot be committed.
    pub fn restore_card_history(
        &mut self,
        card_id: &str,
        baseline: &ScheduleState,
        current: &ScheduleState,
        events: &[ReviewEvent],
    ) -> Result<(), StorageError> {
        let transaction = self.connection.transaction()?;
        let stored_baseline = load_schedule_row(&transaction, "schedule_baselines", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let stored_current = load_schedule_row(&transaction, "schedule_states", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let existing_events = load_review_events(&transaction, card_id)?;
        if stored_baseline != *baseline
            || stored_current != *baseline
            || !existing_events.is_empty()
            || baseline.card_id != card_id
            || current.card_id != card_id
        {
            return Err(StorageError::ProjectionMismatch(
                "portable history requires a pristine card baseline".into(),
            ));
        }

        let projected = project_history(card_id, baseline, events)?;
        if projected != *current {
            return Err(StorageError::ProjectionMismatch(
                "portable current schedule does not match its history".into(),
            ));
        }
        for event in events {
            insert_review_event(&transaction, event)?;
        }

        let changed = transaction.execute(
            "UPDATE schedule_states
             SET version = ?1,
                 due_at_ms = ?2,
                 ideal_due_at_ms = ?3,
                 interval_milliseconds = ?4,
                 interval_seconds = ?5,
                 repetitions = ?6,
                 stability_milliseconds = ?7,
                 difficulty_millipoints = ?8,
                 last_reviewed_at_ms = ?9,
                 last_review_event_id = ?10,
                 lifecycle = ?11
             WHERE card_id = ?12",
            params![
                current.version,
                current.due_at_ms,
                current.ideal_due_at_ms,
                current.interval_milliseconds,
                current.interval_seconds,
                current.repetitions,
                current.stability_milliseconds,
                current.difficulty_millipoints,
                current.last_reviewed_at_ms,
                current.last_review_event_id,
                card_lifecycle_to_database(current.lifecycle),
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
            params![
                events
                    .last()
                    .map_or(current.due_at_ms, |event| event.reviewed_at_ms),
                card_id
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads all immutable review events for a deck in chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deck or stored history cannot be
    /// loaded.
    pub fn review_events_for_deck(&self, deck_id: &str) -> Result<Vec<ReviewEvent>, StorageError> {
        let card_ids = self.card_ids_for_deck(deck_id)?;
        let mut events = Vec::new();
        for card_id in card_ids {
            events.extend(load_review_events(&self.connection, &card_id)?);
        }
        events.sort_by(|left, right| {
            left.reviewed_at_ms
                .cmp(&right.reviewed_at_ms)
                .then_with(|| left.card_id.cmp(&right.card_id))
                .then_with(|| {
                    left.previous_schedule
                        .version
                        .cmp(&right.previous_schedule.version)
                })
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(events)
    }

    /// Loads active graded reviews for a deck in chronological order.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deck or stored history cannot be
    /// loaded.
    pub fn active_review_events_for_deck(
        &self,
        deck_id: &str,
    ) -> Result<Vec<ReviewEvent>, StorageError> {
        let card_ids = self.card_ids_for_deck(deck_id)?;
        let mut events = Vec::new();
        for card_id in card_ids {
            events.extend(active_reviews(load_review_events(
                &self.connection,
                &card_id,
            )?)?);
        }
        events.sort_by(|left, right| {
            left.reviewed_at_ms
                .cmp(&right.reviewed_at_ms)
                .then_with(|| left.card_id.cmp(&right.card_id))
                .then_with(|| {
                    left.previous_schedule
                        .version
                        .cmp(&right.previous_schedule.version)
                })
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(events)
    }

    /// Loads complete study-card aggregates for one deck.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] when the deck or any aggregate cannot be
    /// loaded.
    pub fn study_cards_for_deck(
        &self,
        deck_id: &str,
    ) -> Result<Vec<crate::StoredStudyCard>, StorageError> {
        self.card_ids_for_deck(deck_id)?
            .into_iter()
            .map(|card_id| self.load_study_card(&card_id))
            .collect()
    }

    /// Validates immutable history and current projections for every card.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::ProjectionMismatch`] when recorded history is
    /// malformed, or another [`StorageError`] when collection data cannot be
    /// loaded.
    pub fn check_collection_schedule_integrity(
        &self,
    ) -> Result<ScheduleIntegrityReport, StorageError> {
        let card_ids = all_card_ids(&self.connection)?;
        check_schedule_integrity(&self.connection, &card_ids)
    }

    /// Validates immutable history and current projections for one deck.
    ///
    /// Trashed notes remain part of integrity validation even though they are
    /// not eligible for study.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::ProjectionMismatch`] when recorded history is
    /// malformed, or another [`StorageError`] when the deck cannot be loaded.
    pub fn check_deck_schedule_integrity(
        &self,
        deck_id: &str,
    ) -> Result<ScheduleIntegrityReport, StorageError> {
        let card_ids = integrity_card_ids_for_deck(&self.connection, deck_id)?;
        check_schedule_integrity(&self.connection, &card_ids)
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
        let current = load_schedule_row(&transaction, "schedule_states", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
        let (_, _, projected, queue_updated_at_ms) =
            load_recorded_projection(&transaction, card_id)?;
        if current != projected {
            write_schedule_projection(&transaction, &projected, queue_updated_at_ms)?;
        }
        transaction.commit()?;
        Ok(projected)
    }

    fn card_ids_for_deck(&self, deck_id: &str) -> Result<Vec<String>, StorageError> {
        let exists = self
            .connection
            .query_row("SELECT 1 FROM decks WHERE id = ?1", [deck_id], |_| Ok(()))
            .optional()?
            .is_some();
        if !exists {
            return Err(crate::entity_not_found("deck", deck_id));
        }
        let mut statement = self.connection.prepare(
            "SELECT cards.id
             FROM cards
             JOIN clozes ON clozes.id = cards.cloze_id
             JOIN source_items ON source_items.id = clozes.source_item_id
             WHERE source_items.deck_id = ?1
               AND source_items.deleted_at_ms IS NULL
             ORDER BY cards.id",
        )?;
        Ok(statement
            .query_map([deck_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?)
    }
}

pub(crate) fn repair_all_schedule_projections(
    transaction: &Transaction<'_>,
) -> Result<usize, StorageError> {
    let card_ids = all_card_ids(transaction)?;
    let mut repaired = 0;
    for card_id in card_ids {
        let current = load_schedule_row(transaction, "schedule_states", &card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.clone()))?;
        let (_, _, projected, queue_updated_at_ms) =
            load_recorded_projection(transaction, &card_id)?;
        if current != projected {
            write_schedule_projection(transaction, &projected, queue_updated_at_ms)?;
            repaired += 1;
        }
    }
    Ok(repaired)
}

fn check_schedule_integrity(
    connection: &Connection,
    card_ids: &[String],
) -> Result<ScheduleIntegrityReport, StorageError> {
    let mut mismatched_card_ids = Vec::new();
    for card_id in card_ids {
        let current = load_schedule_row(connection, "schedule_states", card_id)?
            .ok_or_else(|| StorageError::CardNotFound(card_id.clone()))?;
        let (_, _, projected, _) = load_recorded_projection(connection, card_id)?;
        if current != projected {
            mismatched_card_ids.push(card_id.clone());
        }
    }
    Ok(ScheduleIntegrityReport {
        checked_cards: card_ids.len(),
        mismatched_card_ids,
    })
}

fn all_card_ids(connection: &Connection) -> Result<Vec<String>, StorageError> {
    let mut statement = connection.prepare("SELECT id FROM cards ORDER BY id")?;
    Ok(statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn integrity_card_ids_for_deck(
    connection: &Connection,
    deck_id: &str,
) -> Result<Vec<String>, StorageError> {
    let exists = connection
        .query_row("SELECT 1 FROM decks WHERE id = ?1", [deck_id], |_| Ok(()))
        .optional()?
        .is_some();
    if !exists {
        return Err(crate::entity_not_found("deck", deck_id));
    }
    let mut statement = connection.prepare(
        "SELECT cards.id
         FROM cards
         JOIN clozes ON clozes.id = cards.cloze_id
         JOIN source_items ON source_items.id = clozes.source_item_id
         WHERE source_items.deck_id = ?1
         ORDER BY cards.id",
    )?;
    Ok(statement
        .query_map([deck_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?)
}

fn load_recorded_projection(
    connection: &Connection,
    card_id: &str,
) -> Result<(ScheduleState, Vec<ReviewEvent>, ScheduleState, i64), StorageError> {
    let baseline = load_schedule_row(connection, "schedule_baselines", card_id)?
        .ok_or_else(|| StorageError::CardNotFound(card_id.to_owned()))?;
    let events = load_review_events(connection, card_id)?;
    let projected = project_history(card_id, &baseline, &events)?;
    let queue_updated_at_ms = events
        .last()
        .map_or(baseline.due_at_ms, |event| event.reviewed_at_ms);
    Ok((baseline, events, projected, queue_updated_at_ms))
}

fn project_history(
    card_id: &str,
    baseline: &ScheduleState,
    events: &[ReviewEvent],
) -> Result<ScheduleState, StorageError> {
    if baseline.card_id != card_id
        || baseline.version != 0
        || baseline.last_review_event_id.is_some()
    {
        return Err(StorageError::ProjectionMismatch(format!(
            "card {card_id} has an invalid schedule baseline"
        )));
    }

    let mut projected = baseline.clone();
    let mut active_reviews = Vec::<(String, ScheduleState)>::new();
    let mut event_ids = std::collections::HashSet::new();
    for event in events {
        if event.id.trim().is_empty()
            || event.card_id != card_id
            || event.previous_schedule != projected
            || event.next_schedule.card_id != card_id
            || event.previous_schedule.version.checked_add(1) != Some(event.next_schedule.version)
            || event.next_schedule.last_review_event_id.as_deref() != Some(event.id.as_str())
            || !event_ids.insert(event.id.as_str())
        {
            return Err(StorageError::ProjectionMismatch(format!(
                "event {} does not continue card {card_id} version {}",
                event.id, projected.version
            )));
        }
        validate_restored_lifecycle(&mut active_reviews, baseline.lifecycle, event)?;
        projected = event.next_schedule.clone();
    }
    Ok(projected)
}

fn write_schedule_projection(
    transaction: &Transaction<'_>,
    schedule: &ScheduleState,
    queue_updated_at_ms: i64,
) -> Result<(), StorageError> {
    let changed = transaction.execute(
        "UPDATE schedule_states
         SET version = ?1,
             due_at_ms = ?2,
             ideal_due_at_ms = ?3,
             interval_milliseconds = ?4,
             interval_seconds = ?5,
             repetitions = ?6,
             stability_milliseconds = ?7,
             difficulty_millipoints = ?8,
             last_reviewed_at_ms = ?9,
             last_review_event_id = ?10,
             lifecycle = ?11
         WHERE card_id = ?12",
        params![
            schedule.version,
            schedule.due_at_ms,
            schedule.ideal_due_at_ms,
            schedule.interval_milliseconds,
            schedule.interval_seconds,
            schedule.repetitions,
            schedule.stability_milliseconds,
            schedule.difficulty_millipoints,
            schedule.last_reviewed_at_ms,
            schedule.last_review_event_id,
            card_lifecycle_to_database(schedule.lifecycle),
            schedule.card_id,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::CardNotFound(schedule.card_id.clone()));
    }
    let changed = transaction.execute(
        "UPDATE cards
         SET queue_updated_at_ms = ?1
         WHERE id = ?2",
        params![queue_updated_at_ms, schedule.card_id],
    )?;
    if changed != 1 {
        return Err(StorageError::CardNotFound(schedule.card_id.clone()));
    }
    Ok(())
}

fn validate_restored_lifecycle(
    active_reviews: &mut Vec<(String, ScheduleState)>,
    baseline_lifecycle: CardLifecycle,
    event: &ReviewEvent,
) -> Result<(), StorageError> {
    match event.kind {
        ReviewEventKind::Review if event.undoes_review_event_id.is_none() => {
            active_reviews.push((event.id.clone(), event.previous_schedule.clone()));
        }
        ReviewEventKind::Undo => {
            let Some((target_id, prior_schedule)) = active_reviews.last() else {
                return Err(StorageError::ProjectionMismatch(
                    "review history has an invalid compensation chain".into(),
                ));
            };
            if Some(target_id.as_str()) != event.undoes_review_event_id.as_deref() {
                return Err(StorageError::ProjectionMismatch(
                    "review history has an invalid compensation chain".into(),
                ));
            }
            let mut restored = prior_schedule.clone();
            restored.version = event.next_schedule.version;
            restored.last_review_event_id = Some(event.id.clone());
            if event.next_schedule != restored {
                return Err(StorageError::ProjectionMismatch(
                    "undo event does not restore the compensated review snapshot".into(),
                ));
            }
            active_reviews.pop();
        }
        ReviewEventKind::Review => {
            return Err(StorageError::ProjectionMismatch(
                "review history has an invalid compensation chain".into(),
            ));
        }
    }
    let expected = if active_reviews.is_empty() {
        baseline_lifecycle
    } else {
        CardLifecycle::Introduced
    };
    if event.next_schedule.lifecycle != expected {
        return Err(StorageError::ProjectionMismatch(
            "review history has an invalid lifecycle transition".into(),
        ));
    }
    Ok(())
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
    let (baseline, mut events, projected, _) =
        load_recorded_projection(transaction, &event.card_id)?;
    if stored_schedule != projected {
        return Err(StorageError::ProjectionMismatch(format!(
            "card {} current projection does not match immutable history",
            event.card_id
        )));
    }

    if event.previous_schedule.card_id != event.card_id
        || event.next_schedule.card_id != event.card_id
        || event.next_schedule.last_review_event_id.as_deref() != Some(event.id.as_str())
        || (event.kind == ReviewEventKind::Review
            && event.next_schedule.lifecycle != CardLifecycle::Introduced)
        || (event.kind == ReviewEventKind::Review && event.undoes_review_event_id.is_some())
        || (event.kind == ReviewEventKind::Undo && event.undoes_review_event_id.is_none())
        || card_version != event.card_content_version
        || stored_schedule != event.previous_schedule
        || event.previous_schedule.version.checked_add(1) != Some(event.next_schedule.version)
    {
        return Err(StorageError::StaleReview);
    }
    events.push(event.clone());
    if project_history(&event.card_id, &baseline, &events)? != event.next_schedule {
        return Err(StorageError::ProjectionMismatch(format!(
            "event {} does not produce its recorded projection",
            event.id
        )));
    }
    Ok(())
}

fn persist_review_event(
    transaction: &Transaction<'_>,
    event: &ReviewEvent,
) -> Result<(), StorageError> {
    validate_review_preconditions(transaction, event)?;
    insert_review_event(transaction, event)?;

    let changed = transaction.execute(
        "UPDATE schedule_states
         SET version = ?1,
             due_at_ms = ?2,
             ideal_due_at_ms = ?3,
             interval_milliseconds = ?4,
             interval_seconds = ?5,
             repetitions = ?6,
             stability_milliseconds = ?7,
             difficulty_millipoints = ?8,
             last_reviewed_at_ms = ?9,
             last_review_event_id = ?10,
             lifecycle = ?11
         WHERE card_id = ?12
           AND version = ?13
           AND due_at_ms = ?14
           AND ideal_due_at_ms = ?15
           AND interval_milliseconds = ?16
           AND interval_seconds = ?17
           AND repetitions = ?18
           AND stability_milliseconds = ?19
           AND difficulty_millipoints = ?20
           AND last_reviewed_at_ms IS ?21
           AND last_review_event_id IS ?22
           AND lifecycle = ?23",
        params![
            event.next_schedule.version,
            event.next_schedule.due_at_ms,
            event.next_schedule.ideal_due_at_ms,
            event.next_schedule.interval_milliseconds,
            event.next_schedule.interval_seconds,
            event.next_schedule.repetitions,
            event.next_schedule.stability_milliseconds,
            event.next_schedule.difficulty_millipoints,
            event.next_schedule.last_reviewed_at_ms,
            event.id,
            card_lifecycle_to_database(event.next_schedule.lifecycle),
            event.card_id,
            event.previous_schedule.version,
            event.previous_schedule.due_at_ms,
            event.previous_schedule.ideal_due_at_ms,
            event.previous_schedule.interval_milliseconds,
            event.previous_schedule.interval_seconds,
            event.previous_schedule.repetitions,
            event.previous_schedule.stability_milliseconds,
            event.previous_schedule.difficulty_millipoints,
            event.previous_schedule.last_reviewed_at_ms,
            event.previous_schedule.last_review_event_id,
            card_lifecycle_to_database(event.previous_schedule.lifecycle),
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
    Ok(())
}

fn active_reviews(events: Vec<ReviewEvent>) -> Result<Vec<ReviewEvent>, StorageError> {
    let mut active = Vec::<ReviewEvent>::new();
    for event in events {
        match event.kind {
            ReviewEventKind::Review => active.push(event),
            ReviewEventKind::Undo => {
                let target = event.undoes_review_event_id.as_deref().ok_or_else(|| {
                    StorageError::ProjectionMismatch(format!(
                        "undo event {} has no review reference",
                        event.id
                    ))
                })?;
                if active.last().map(|review| review.id.as_str()) != Some(target) {
                    return Err(StorageError::ProjectionMismatch(format!(
                        "undo event {} does not compensate the latest active review",
                        event.id
                    )));
                }
                active.pop();
            }
        }
    }
    Ok(active)
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
            scheduler_parameter_set_id,
            target_retention_basis_points,
            previous_schedule_version,
            previous_due_at_ms,
            previous_ideal_due_at_ms,
            previous_interval_milliseconds,
            previous_interval_seconds,
            previous_repetitions,
            previous_stability_milliseconds,
            previous_difficulty_millipoints,
            previous_last_reviewed_at_ms,
            next_schedule_version,
            next_due_at_ms,
            next_ideal_due_at_ms,
            next_interval_milliseconds,
            next_interval_seconds,
            next_repetitions,
            next_stability_milliseconds,
            next_difficulty_millipoints,
            next_last_reviewed_at_ms,
            event_kind,
            undoes_review_event_id,
            response_duration_ms,
            grade_overridden,
            previous_lifecycle,
            next_lifecycle
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20,
            ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30,
            ?31, ?32, ?33, ?34, ?35, ?36
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
            event.scheduler_parameter_set_id,
            event.target_retention_basis_points,
            event.previous_schedule.version,
            event.previous_schedule.due_at_ms,
            event.previous_schedule.ideal_due_at_ms,
            event.previous_schedule.interval_milliseconds,
            event.previous_schedule.interval_seconds,
            event.previous_schedule.repetitions,
            event.previous_schedule.stability_milliseconds,
            event.previous_schedule.difficulty_millipoints,
            event.previous_schedule.last_reviewed_at_ms,
            event.next_schedule.version,
            event.next_schedule.due_at_ms,
            event.next_schedule.ideal_due_at_ms,
            event.next_schedule.interval_milliseconds,
            event.next_schedule.interval_seconds,
            event.next_schedule.repetitions,
            event.next_schedule.stability_milliseconds,
            event.next_schedule.difficulty_millipoints,
            event.next_schedule.last_reviewed_at_ms,
            review_event_kind_to_database(event.kind),
            event.undoes_review_event_id,
            event.response_duration_ms,
            event.grade_overridden,
            card_lifecycle_to_database(event.previous_schedule.lifecycle),
            card_lifecycle_to_database(event.next_schedule.lifecycle),
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
            scheduler_parameter_set_id,
            target_retention_basis_points,
            previous_schedule_version,
            previous_due_at_ms,
            previous_ideal_due_at_ms,
            previous_interval_milliseconds,
            previous_interval_seconds,
            previous_repetitions,
            previous_stability_milliseconds,
            previous_difficulty_millipoints,
            previous_last_reviewed_at_ms,
            next_schedule_version,
            next_due_at_ms,
            next_ideal_due_at_ms,
            next_interval_milliseconds,
            next_interval_seconds,
            next_repetitions,
            next_stability_milliseconds,
            next_difficulty_millipoints,
            next_last_reviewed_at_ms,
            event_kind,
            undoes_review_event_id,
            response_duration_ms,
            grade_overridden,
            previous_lifecycle,
            next_lifecycle
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
    scheduler_parameter_set_id: Option<String>,
    target_retention_basis_points: u16,
    previous_version: u64,
    previous_due_at_ms: i64,
    previous_ideal_due_at_ms: i64,
    previous_interval_milliseconds: u64,
    previous_interval_seconds: u64,
    previous_repetitions: u32,
    previous_stability_milliseconds: u64,
    previous_difficulty_millipoints: u32,
    previous_last_reviewed_at_ms: Option<i64>,
    next_version: u64,
    next_due_at_ms: i64,
    next_ideal_due_at_ms: i64,
    next_interval_milliseconds: u64,
    next_interval_seconds: u64,
    next_repetitions: u32,
    next_stability_milliseconds: u64,
    next_difficulty_millipoints: u32,
    next_last_reviewed_at_ms: Option<i64>,
    event_kind: String,
    undoes_review_event_id: Option<String>,
    response_duration_ms: u64,
    grade_overridden: bool,
    previous_lifecycle: String,
    next_lifecycle: String,
}

impl StoredReviewEvent {
    fn into_domain(self, previous_event_id: Option<String>) -> Result<ReviewEvent, StorageError> {
        let is_legacy = self.scheduler_version != "fsrs-7";
        let kind = review_event_kind_from_database(&self.event_kind)?;
        let grade_overridden = self.grade_overridden
            || (kind == ReviewEventKind::Review && self.chosen_grade != self.suggested_grade);
        let previous_schedule = ScheduleState {
            card_id: self.card_id.clone(),
            version: self.previous_version,
            lifecycle: card_lifecycle_from_database(&self.previous_lifecycle)?,
            due_at_ms: self.previous_due_at_ms,
            ideal_due_at_ms: if is_legacy {
                self.previous_due_at_ms
            } else {
                self.previous_ideal_due_at_ms
            },
            interval_milliseconds: if is_legacy {
                self.previous_interval_seconds.saturating_mul(1_000)
            } else {
                self.previous_interval_milliseconds
            },
            interval_seconds: self.previous_interval_seconds,
            repetitions: self.previous_repetitions,
            stability_milliseconds: self.previous_stability_milliseconds,
            difficulty_millipoints: self.previous_difficulty_millipoints,
            last_reviewed_at_ms: self.previous_last_reviewed_at_ms,
            last_review_event_id: previous_event_id,
        };
        let next_schedule = ScheduleState {
            card_id: self.card_id.clone(),
            version: self.next_version,
            lifecycle: card_lifecycle_from_database(&self.next_lifecycle)?,
            due_at_ms: self.next_due_at_ms,
            ideal_due_at_ms: if is_legacy {
                self.next_due_at_ms
            } else {
                self.next_ideal_due_at_ms
            },
            interval_milliseconds: if is_legacy {
                self.next_interval_seconds.saturating_mul(1_000)
            } else {
                self.next_interval_milliseconds
            },
            interval_seconds: self.next_interval_seconds,
            repetitions: self.next_repetitions,
            stability_milliseconds: self.next_stability_milliseconds,
            difficulty_millipoints: self.next_difficulty_millipoints,
            last_reviewed_at_ms: self.next_last_reviewed_at_ms,
            last_review_event_id: Some(self.id.clone()),
        };
        Ok(ReviewEvent {
            id: self.id,
            card_id: self.card_id,
            card_content_version: self.card_content_version,
            kind,
            undoes_review_event_id: self.undoes_review_event_id,
            raw_response: self.raw_response,
            normalized_response: self.normalized_response,
            comparison: comparison_from_database(&self.comparison)?,
            suggested_grade: grade_from_database(&self.suggested_grade)?,
            chosen_grade: grade_from_database(&self.chosen_grade)?,
            grade_overridden,
            response_duration_ms: self.response_duration_ms,
            reviewed_at_ms: self.reviewed_at_ms,
            scheduler_version: self.scheduler_version,
            scheduler_parameter_set_id: self.scheduler_parameter_set_id,
            target_retention_basis_points: self.target_retention_basis_points,
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
        scheduler_parameter_set_id: row.get(10)?,
        target_retention_basis_points: row.get(11)?,
        previous_version: row.get(12)?,
        previous_due_at_ms: row.get(13)?,
        previous_ideal_due_at_ms: row.get(14)?,
        previous_interval_milliseconds: row.get(15)?,
        previous_interval_seconds: row.get(16)?,
        previous_repetitions: row.get(17)?,
        previous_stability_milliseconds: row.get(18)?,
        previous_difficulty_millipoints: row.get(19)?,
        previous_last_reviewed_at_ms: row.get(20)?,
        next_version: row.get(21)?,
        next_due_at_ms: row.get(22)?,
        next_ideal_due_at_ms: row.get(23)?,
        next_interval_milliseconds: row.get(24)?,
        next_interval_seconds: row.get(25)?,
        next_repetitions: row.get(26)?,
        next_stability_milliseconds: row.get(27)?,
        next_difficulty_millipoints: row.get(28)?,
        next_last_reviewed_at_ms: row.get(29)?,
        event_kind: row.get(30)?,
        undoes_review_event_id: row.get(31)?,
        response_duration_ms: row.get(32)?,
        grade_overridden: row.get(33)?,
        previous_lifecycle: row.get(34)?,
        next_lifecycle: row.get(35)?,
    })
}

const fn review_event_kind_to_database(value: ReviewEventKind) -> &'static str {
    match value {
        ReviewEventKind::Review => "review",
        ReviewEventKind::Undo => "undo",
    }
}

fn review_event_kind_from_database(value: &str) -> Result<ReviewEventKind, StorageError> {
    match value {
        "review" => Ok(ReviewEventKind::Review),
        "undo" => Ok(ReviewEventKind::Undo),
        _ => Err(StorageError::InvalidStoredValue {
            field: "review event kind",
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
