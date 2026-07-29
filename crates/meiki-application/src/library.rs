use std::{
    collections::{HashMap, HashSet},
    fs,
};

use meiki_domain::{Cloze, Deck, SegmentContent, SourceItem, Tag};
use meiki_storage::{DeckRepository, StoredLibraryNote, TagRepository};
use meiki_text::normalize_for_search;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{ApplicationError, ApplicationService, DirectionDto, timestamp_string};

const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 200;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum LibraryDueFilterDto {
    #[default]
    All,
    Due,
    New,
    Scheduled,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum LibrarySuspendedFilterDto {
    #[default]
    All,
    Active,
    Suspended,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum LibraryMediaFilterDto {
    #[default]
    All,
    WithMedia,
    WithoutMedia,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum LibraryTrashFilterDto {
    #[default]
    Active,
    Deleted,
    All,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryRequest {
    pub query: String,
    pub deck_id: Option<String>,
    pub tag_id: Option<String>,
    pub due: LibraryDueFilterDto,
    pub suspended: LibrarySuspendedFilterDto,
    pub language_tag: Option<String>,
    pub media: LibraryMediaFilterDto,
    pub trash: LibraryTrashFilterDto,
    #[ts(type = "number")]
    pub now_ms: i64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryDeckDto {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryTagDto {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[allow(clippy::struct_excessive_bools)]
pub struct LibraryCardDto {
    pub card_id: String,
    pub cloze_id: String,
    pub prompt: String,
    pub answer: String,
    pub suspended: bool,
    pub is_new: bool,
    pub is_due: bool,
    pub due_at: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub has_media: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryNoteDto {
    pub source_id: String,
    pub deck_id: String,
    pub deck_name: String,
    pub source_text: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub tags: Vec<LibraryTagDto>,
    pub cards: Vec<LibraryCardDto>,
    pub media_count: u32,
    pub deleted: bool,
    pub deleted_at: Option<String>,
    #[ts(type = "number")]
    pub updated_at_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryOverviewDto {
    pub notes: Vec<LibraryNoteDto>,
    pub decks: Vec<LibraryDeckDto>,
    pub tags: Vec<LibraryTagDto>,
    pub languages: Vec<String>,
    pub total_matches: u32,
    pub active_notes: u32,
    pub trashed_notes: u32,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum LibraryBulkActionDto {
    Suspend,
    Unsuspend,
    Delete,
    Restore,
    Move,
    AddTag,
    RemoveTag,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryBulkRequest {
    pub source_ids: Vec<String>,
    pub action: LibraryBulkActionDto,
    pub deck_id: Option<String>,
    pub tag_id: Option<String>,
    pub tag_name: Option<String>,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryBulkResultDto {
    pub affected_notes: u32,
    pub action: LibraryBulkActionDto,
    pub undo_action: Option<LibraryBulkActionDto>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryExportRequest {
    pub source_ids: Vec<String>,
    #[ts(type = "number")]
    pub now_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct LibraryExportResultDto {
    pub path: String,
    pub exported_notes: u32,
}

#[derive(Clone)]
struct LibraryRecord {
    stored: StoredLibraryNote,
    deck: Deck,
}

#[derive(Serialize)]
struct LibrarySelectionExport {
    schema_version: u32,
    exported_at_ms: i64,
    notes: Vec<LibraryNoteDto>,
}

impl ApplicationService {
    /// Searches and filters a deterministic page of source notes.
    ///
    /// # Errors
    ///
    /// Returns an error when collection data is invalid or cannot be loaded.
    pub fn get_library(
        &self,
        request: &LibraryRequest,
    ) -> Result<LibraryOverviewDto, ApplicationError> {
        validate_library_request(request)?;
        let storage = self.open_storage()?;
        let decks = storage.list_decks()?;
        let by_deck = decks
            .iter()
            .cloned()
            .map(|deck| (deck.id.clone(), deck))
            .collect::<HashMap<_, _>>();
        let records = storage
            .library_notes()?
            .into_iter()
            .map(|stored| {
                let deck = by_deck
                    .get(&stored.note.source_item.deck_id)
                    .cloned()
                    .ok_or_else(|| {
                        ApplicationError::InvalidLibrary(
                            "a source note references a missing deck".to_owned(),
                        )
                    })?;
                Ok(LibraryRecord { stored, deck })
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?;
        let active_notes = records
            .iter()
            .filter(|record| record.stored.deleted_at_ms.is_none())
            .count();
        let trashed_notes = records.len().saturating_sub(active_notes);
        let mut languages = language_options(&records);
        languages.sort();

        let normalized_query = normalize_for_search(&request.query);
        let matched = records
            .iter()
            .filter(|record| library_matches(record, request, &normalized_query))
            .collect::<Vec<_>>();
        let total_matches = matched.len();
        let offset = usize::try_from(request.offset).unwrap_or(usize::MAX);
        let limit = page_size(request.limit);
        let notes = matched
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|record| library_note_dto(record, request.now_ms))
            .collect::<Result<Vec<_>, ApplicationError>>()?;

        Ok(LibraryOverviewDto {
            notes,
            decks: decks
                .into_iter()
                .map(|deck| LibraryDeckDto {
                    id: deck.id,
                    name: deck.name,
                })
                .collect(),
            tags: storage
                .list_tags()?
                .into_iter()
                .map(|tag| LibraryTagDto {
                    id: tag.id,
                    name: tag.name,
                })
                .collect(),
            languages,
            total_matches: desktop_count(total_matches, "library match count")?,
            active_notes: desktop_count(active_notes, "active library note count")?,
            trashed_notes: desktop_count(trashed_notes, "trashed library note count")?,
            offset: request.offset,
            limit: u32::try_from(limit)
                .map_err(|_| ApplicationError::NumericRange("library page size"))?,
        })
    }

    /// Applies one action to all selected notes in one storage transaction.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection or action parameters are invalid,
    /// any selected note is missing, or the transaction cannot be committed.
    pub fn apply_library_bulk_action(
        &self,
        request: &LibraryBulkRequest,
    ) -> Result<LibraryBulkResultDto, ApplicationError> {
        validate_source_ids(&request.source_ids)?;
        let mut storage = self.open_storage()?;
        let undo_action = match request.action {
            LibraryBulkActionDto::Suspend => {
                storage.set_library_notes_suspended(&request.source_ids, true, request.now_ms)?;
                Some(LibraryBulkActionDto::Unsuspend)
            }
            LibraryBulkActionDto::Unsuspend => {
                storage.set_library_notes_suspended(&request.source_ids, false, request.now_ms)?;
                Some(LibraryBulkActionDto::Suspend)
            }
            LibraryBulkActionDto::Delete => {
                storage.set_library_notes_deleted(
                    &request.source_ids,
                    Some(request.now_ms),
                    request.now_ms,
                )?;
                Some(LibraryBulkActionDto::Restore)
            }
            LibraryBulkActionDto::Restore => {
                storage.set_library_notes_deleted(&request.source_ids, None, request.now_ms)?;
                Some(LibraryBulkActionDto::Delete)
            }
            LibraryBulkActionDto::Move => {
                let deck_id = required_value(request.deck_id.as_deref(), "destination deck")?;
                storage.move_library_notes(&request.source_ids, deck_id, request.now_ms)?;
                None
            }
            LibraryBulkActionDto::AddTag => {
                let name = required_value(request.tag_name.as_deref(), "tag name")?.trim();
                let tag = storage
                    .list_tags()?
                    .into_iter()
                    .find(|tag| normalize_for_search(&tag.name) == normalize_for_search(name))
                    .unwrap_or_else(|| Tag {
                        id: Uuid::new_v4().to_string(),
                        name: name.to_owned(),
                        created_at_ms: request.now_ms,
                        updated_at_ms: request.now_ms,
                    });
                storage.tag_library_notes(&request.source_ids, &tag, request.now_ms)?;
                None
            }
            LibraryBulkActionDto::RemoveTag => {
                let tag_id = required_value(request.tag_id.as_deref(), "tag")?;
                storage.untag_library_notes(&request.source_ids, tag_id, request.now_ms)?;
                None
            }
        };
        Ok(LibraryBulkResultDto {
            affected_notes: desktop_count(request.source_ids.len(), "affected library note count")?,
            action: request.action,
            undo_action,
        })
    }

    /// Writes a selected-note JSON interchange snapshot without modifying data.
    ///
    /// This lightweight selection export is intentionally distinct from the
    /// complete versioned archive implemented by the portability boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when selection data cannot be loaded, serialized, or
    /// written to the local exports directory.
    pub fn export_library_selection(
        &self,
        request: &LibraryExportRequest,
    ) -> Result<LibraryExportResultDto, ApplicationError> {
        validate_source_ids(&request.source_ids)?;
        let selected = request.source_ids.iter().collect::<HashSet<_>>();
        let storage = self.open_storage()?;
        let decks = storage
            .list_decks()?
            .into_iter()
            .map(|deck| (deck.id.clone(), deck))
            .collect::<HashMap<_, _>>();
        let notes = storage
            .library_notes()?
            .into_iter()
            .filter(|stored| selected.contains(&stored.note.source_item.id))
            .map(|stored| {
                let deck = decks
                    .get(&stored.note.source_item.deck_id)
                    .cloned()
                    .ok_or_else(|| {
                        ApplicationError::InvalidLibrary(
                            "a source note references a missing deck".to_owned(),
                        )
                    })?;
                library_note_dto(&LibraryRecord { stored, deck }, request.now_ms)
            })
            .collect::<Result<Vec<_>, ApplicationError>>()?;
        if notes.len() != selected.len() {
            return Err(ApplicationError::InvalidLibrary(
                "one or more selected source notes no longer exist".to_owned(),
            ));
        }
        let export = LibrarySelectionExport {
            schema_version: 1,
            exported_at_ms: request.now_ms,
            notes,
        };
        let directory = self
            .collection_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("exports");
        fs::create_dir_all(&directory).map_err(ApplicationError::LibraryExport)?;
        let path = directory.join(format!("library-selection-{}.json", Uuid::new_v4()));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&export).map_err(ApplicationError::LibrarySerialization)?,
        )
        .map_err(ApplicationError::LibraryExport)?;
        Ok(LibraryExportResultDto {
            path: path.display().to_string(),
            exported_notes: desktop_count(export.notes.len(), "exported library note count")?,
        })
    }
}

fn validate_library_request(request: &LibraryRequest) -> Result<(), ApplicationError> {
    if request.limit > u32::try_from(MAX_PAGE_SIZE).unwrap_or(u32::MAX)
        || request
            .deck_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || request
            .tag_id
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        || request
            .language_tag
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
    {
        return Err(ApplicationError::InvalidLibrary(
            "library filters or page bounds are invalid".to_owned(),
        ));
    }
    Ok(())
}

fn validate_source_ids(source_ids: &[String]) -> Result<(), ApplicationError> {
    let unique = source_ids.iter().collect::<HashSet<_>>();
    if source_ids.is_empty()
        || unique.len() != source_ids.len()
        || source_ids.iter().any(|id| id.trim().is_empty())
    {
        return Err(ApplicationError::InvalidLibrary(
            "select one or more distinct source notes".to_owned(),
        ));
    }
    Ok(())
}

fn required_value<'a>(
    value: Option<&'a str>,
    field: &'static str,
) -> Result<&'a str, ApplicationError> {
    value
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ApplicationError::InvalidLibrary(format!("{field} is required")))
}

fn page_size(requested: u32) -> usize {
    if requested == 0 {
        DEFAULT_PAGE_SIZE
    } else {
        usize::try_from(requested).unwrap_or(DEFAULT_PAGE_SIZE)
    }
}

fn library_matches(
    record: &LibraryRecord,
    request: &LibraryRequest,
    normalized_query: &str,
) -> bool {
    let source = &record.stored.note.source_item;
    let deleted = record.stored.deleted_at_ms.is_some();
    let trash_matches = match request.trash {
        LibraryTrashFilterDto::Active => !deleted,
        LibraryTrashFilterDto::Deleted => deleted,
        LibraryTrashFilterDto::All => true,
    };
    trash_matches
        && request
            .deck_id
            .as_ref()
            .is_none_or(|deck_id| source.deck_id == *deck_id)
        && request
            .tag_id
            .as_ref()
            .is_none_or(|tag_id| source.tags.iter().any(|tag| tag.id == *tag_id))
        && request
            .language_tag
            .as_ref()
            .is_none_or(|language| record_languages(record).any(|stored| stored == language))
        && match request.media {
            LibraryMediaFilterDto::All => true,
            LibraryMediaFilterDto::WithMedia => media_count(record) > 0,
            LibraryMediaFilterDto::WithoutMedia => media_count(record) == 0,
        }
        && match request.suspended {
            LibrarySuspendedFilterDto::All => true,
            LibrarySuspendedFilterDto::Active => {
                record.stored.cards.iter().any(|card| !card.card.suspended)
            }
            LibrarySuspendedFilterDto::Suspended => {
                record.stored.cards.iter().any(|card| card.card.suspended)
            }
        }
        && match request.due {
            LibraryDueFilterDto::All => true,
            LibraryDueFilterDto::Due => record.stored.cards.iter().any(|card| {
                card.schedule.repetitions > 0 && card.schedule.due_at_ms <= request.now_ms
            }),
            LibraryDueFilterDto::New => record
                .stored
                .cards
                .iter()
                .any(|card| card.schedule.repetitions == 0),
            LibraryDueFilterDto::Scheduled => record.stored.cards.iter().any(|card| {
                card.schedule.repetitions > 0 && card.schedule.due_at_ms > request.now_ms
            }),
        }
        && (normalized_query.is_empty()
            || search_values(record)
                .any(|value| normalize_for_search(value).contains(normalized_query)))
}

fn library_note_dto(
    record: &LibraryRecord,
    now_ms: i64,
) -> Result<LibraryNoteDto, ApplicationError> {
    let source = &record.stored.note.source_item;
    let cards = record
        .stored
        .note
        .clozes
        .iter()
        .zip(&record.stored.cards)
        .map(|(cloze, stored)| {
            Ok(LibraryCardDto {
                card_id: stored.card.id.clone(),
                cloze_id: cloze.id.clone(),
                prompt: prompt(source, &cloze.id),
                answer: cloze.answer.clone(),
                suspended: stored.card.suspended,
                is_new: stored.schedule.repetitions == 0,
                is_due: stored.schedule.repetitions > 0 && stored.schedule.due_at_ms <= now_ms,
                due_at: timestamp_string(stored.schedule.due_at_ms)?,
                language_tag: cloze
                    .language_tag
                    .clone()
                    .or_else(|| source.language_tag.clone())
                    .or_else(|| record.deck.language_tag.clone()),
                direction: resolved_direction(cloze, source, &record.deck),
                has_media: !cloze.media.is_empty(),
            })
        })
        .collect::<Result<Vec<_>, ApplicationError>>()?;
    Ok(LibraryNoteDto {
        source_id: source.id.clone(),
        deck_id: source.deck_id.clone(),
        deck_name: record.deck.name.clone(),
        source_text: source_text(source),
        language_tag: source
            .language_tag
            .clone()
            .or_else(|| record.deck.language_tag.clone()),
        direction: source.direction.into(),
        tags: source
            .tags
            .iter()
            .map(|tag| LibraryTagDto {
                id: tag.id.clone(),
                name: tag.name.clone(),
            })
            .collect(),
        cards,
        media_count: desktop_count(media_count(record), "library media count")?,
        deleted: record.stored.deleted_at_ms.is_some(),
        deleted_at: record
            .stored
            .deleted_at_ms
            .map(timestamp_string)
            .transpose()?,
        updated_at_ms: source.updated_at_ms,
    })
}

fn search_values(record: &LibraryRecord) -> impl Iterator<Item = &str> {
    let source = &record.stored.note.source_item;
    let mut values = Vec::new();
    values.push(record.deck.name.as_str());
    values.extend(
        source
            .segments
            .iter()
            .map(|segment| match &segment.content {
                SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text.as_str(),
            }),
    );
    values.extend(source.tags.iter().map(|tag| tag.name.as_str()));
    values.extend(
        source
            .annotations
            .iter()
            .flat_map(|annotation| [annotation.label.as_str(), annotation.value.as_str()]),
    );
    if let Some(explanation) = &source.explanation {
        values.push(explanation.value.as_str());
    }
    for cloze in &record.stored.note.clozes {
        values.push(cloze.answer.as_str());
        values.extend(cloze.accepted_answers.iter().map(String::as_str));
        if let Some(hint) = &cloze.hint {
            values.push(hint.value.as_str());
        }
        values.extend(
            cloze
                .annotations
                .iter()
                .flat_map(|annotation| [annotation.label.as_str(), annotation.value.as_str()]),
        );
        if let Some(explanation) = &cloze.explanation {
            values.push(explanation.value.as_str());
        }
    }
    values.into_iter()
}

fn record_languages(record: &LibraryRecord) -> impl Iterator<Item = &str> {
    let mut languages = Vec::new();
    if let Some(language) = record.deck.language_tag.as_deref() {
        languages.push(language);
    }
    if let Some(language) = record.stored.note.source_item.language_tag.as_deref() {
        languages.push(language);
    }
    languages.extend(
        record
            .stored
            .note
            .clozes
            .iter()
            .filter_map(|cloze| cloze.language_tag.as_deref()),
    );
    languages.into_iter()
}

fn language_options(records: &[LibraryRecord]) -> Vec<String> {
    records
        .iter()
        .flat_map(record_languages)
        .map(str::to_owned)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn media_count(record: &LibraryRecord) -> usize {
    record.stored.note.source_item.media.len()
        + record
            .stored
            .note
            .clozes
            .iter()
            .map(|cloze| cloze.media.len())
            .sum::<usize>()
}

fn source_text(source: &SourceItem) -> String {
    source
        .segments
        .iter()
        .map(|segment| match &segment.content {
            SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text.as_str(),
        })
        .collect()
}

fn prompt(source: &SourceItem, active_cloze_id: &str) -> String {
    source
        .segments
        .iter()
        .map(|segment| match &segment.content {
            SegmentContent::Cloze { cloze_id, .. } if cloze_id == active_cloze_id => "[…]",
            SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text.as_str(),
        })
        .collect()
}

fn resolved_direction(cloze: &Cloze, source: &SourceItem, deck: &Deck) -> DirectionDto {
    if !matches!(cloze.direction, meiki_domain::Direction::Auto) {
        cloze.direction.into()
    } else if !matches!(source.direction, meiki_domain::Direction::Auto) {
        source.direction.into()
    } else {
        deck.direction.into()
    }
}

fn desktop_count(value: usize, field: &'static str) -> Result<u32, ApplicationError> {
    u32::try_from(value).map_err(|_| ApplicationError::NumericRange(field))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use meiki_domain::{
        Annotation, Direction, LocalizedText, MediaKind, MediaReference, MediaRole, Tag,
    };
    use meiki_storage::{
        DEFAULT_DECK_ID, DeckRepository, MediaRepository, SAMPLE_CARD_ID, SourceNoteRepository,
        Storage,
    };
    use tempfile::tempdir;

    use super::{
        ApplicationService, LibraryBulkActionDto, LibraryBulkRequest, LibraryDueFilterDto,
        LibraryExportRequest, LibraryMediaFilterDto, LibraryRecord, LibraryRequest,
        LibrarySuspendedFilterDto, LibraryTrashFilterDto, library_matches,
    };

    fn request(query: &str) -> LibraryRequest {
        LibraryRequest {
            query: query.to_owned(),
            deck_id: None,
            tag_id: None,
            due: LibraryDueFilterDto::All,
            suspended: LibrarySuspendedFilterDto::All,
            language_tag: None,
            media: LibraryMediaFilterDto::All,
            trash: LibraryTrashFilterDto::Active,
            now_ms: 10_000,
            offset: 0,
            limit: 50,
        }
    }

    fn media() -> MediaReference {
        MediaReference {
            id: "library-media".into(),
            content_hash: "sha256:library-media".into(),
            kind: MediaKind::Image,
            role: MediaRole::RevealImage,
            media_type: "image/png".into(),
            byte_size: 128,
            original_file_name: Some("図書館.png".into()),
            alt_text: Some("مكتبة".into()),
            width: Some(1),
            height: Some(1),
            duration_ms: None,
            language_tag: Some("ar".into()),
            direction: Direction::RightToLeft,
            created_at_ms: 1_000,
        }
    }

    fn service_with_search_fixture() -> (tempfile::TempDir, ApplicationService, String) {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        service.initialize_collection().unwrap();
        let mut storage = Storage::open(&directory.path().join("collection.db")).unwrap();
        let source_id = storage.library_notes().unwrap()[0]
            .note
            .source_item
            .id
            .clone();
        let mut note = storage.get_source_note(&source_id).unwrap();
        note.source_item.tags.push(Tag {
            id: "tag-width".into(),
            name: "ＣＡＦÉ".into(),
            created_at_ms: 1_000,
            updated_at_ms: 1_000,
        });
        note.source_item.annotations.push(Annotation {
            id: "source-annotation".into(),
            label: "Register".into(),
            value: "تعليق عربي".into(),
            language_tag: Some("ar".into()),
            direction: Direction::RightToLeft,
        });
        note.source_item.explanation = Some(LocalizedText {
            value: "संस्कृत explanation".into(),
            language_tag: Some("hi".into()),
            direction: Direction::LeftToRight,
        });
        note.source_item.media.push(media());
        note.clozes[0].accepted_answers.push("ゆきましょう".into());
        note.clozes[0].hint = Some(LocalizedText {
            value: "動詞の丁寧形".into(),
            language_tag: Some("ja".into()),
            direction: Direction::Auto,
        });
        note.clozes[0].annotations.push(Annotation {
            id: "cloze-annotation".into(),
            label: "Grammar".into(),
            value: "פועל".into(),
            language_tag: Some("he".into()),
            direction: Direction::RightToLeft,
        });
        note.clozes[0].explanation = Some(LocalizedText {
            value: "Höfliche Gegenwart".into(),
            language_tag: Some("de".into()),
            direction: Direction::LeftToRight,
        });
        storage.update_source_note(&note).unwrap();
        (directory, service, source_id)
    }

    #[test]
    fn unicode_search_covers_every_library_field_and_combines_filters() {
        let (_directory, service, source_id) = service_with_search_fixture();
        for query in [
            "図書館",
            "行きます",
            "ゆきましょう",
            "動詞",
            "grammar",
            "פועל",
            "höfliche",
            "REGISTER",
            "تعليق",
            "संस्कृत",
            "café",
            "default",
        ] {
            let overview = service.get_library(&request(query)).unwrap();
            assert_eq!(
                overview.notes.len(),
                1,
                "query should discover the fixture: {query}"
            );
            assert_eq!(overview.notes[0].source_id, source_id);
        }

        let overview = service.get_library(&request("不存在")).unwrap();
        assert!(overview.notes.is_empty());

        let mut filtered = request("");
        filtered.deck_id = Some(DEFAULT_DECK_ID.into());
        filtered.tag_id = Some("tag-width".into());
        filtered.due = LibraryDueFilterDto::New;
        filtered.suspended = LibrarySuspendedFilterDto::Active;
        filtered.language_tag = Some("ja".into());
        filtered.media = LibraryMediaFilterDto::WithMedia;
        assert_eq!(service.get_library(&filtered).unwrap().notes.len(), 1);

        filtered.media = LibraryMediaFilterDto::WithoutMedia;
        assert!(service.get_library(&filtered).unwrap().notes.is_empty());
    }

    #[test]
    fn bulk_actions_and_export_do_not_change_review_history() {
        let (directory, service, source_id) = service_with_search_fixture();
        let storage = Storage::open(&directory.path().join("collection.db")).unwrap();
        let history_before = storage.review_count(SAMPLE_CARD_ID).unwrap();
        let schedule_before = storage.load_schedule(SAMPLE_CARD_ID).unwrap();
        drop(storage);

        let suspend = service
            .apply_library_bulk_action(&LibraryBulkRequest {
                source_ids: vec![source_id.clone()],
                action: LibraryBulkActionDto::Suspend,
                deck_id: None,
                tag_id: None,
                tag_name: None,
                now_ms: 20_000,
            })
            .unwrap();
        assert_eq!(suspend.undo_action, Some(LibraryBulkActionDto::Unsuspend));
        let mut suspended = request("");
        suspended.suspended = LibrarySuspendedFilterDto::Suspended;
        assert_eq!(service.get_library(&suspended).unwrap().notes.len(), 1);

        service
            .apply_library_bulk_action(&LibraryBulkRequest {
                source_ids: vec![source_id.clone()],
                action: LibraryBulkActionDto::AddTag,
                deck_id: None,
                tag_id: None,
                tag_name: Some("  旅行  ".into()),
                now_ms: 21_000,
            })
            .unwrap();
        assert_eq!(
            service.get_library(&request("旅行")).unwrap().notes.len(),
            1
        );

        let exported = service
            .export_library_selection(&LibraryExportRequest {
                source_ids: vec![source_id.clone()],
                now_ms: 22_000,
            })
            .unwrap();
        assert_eq!(exported.exported_notes, 1);
        let export: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&exported.path).unwrap()).unwrap();
        assert_eq!(export["schema_version"], 1);
        assert_eq!(export["notes"][0]["source_id"], source_id);

        let deleted = service
            .apply_library_bulk_action(&LibraryBulkRequest {
                source_ids: vec![source_id.clone()],
                action: LibraryBulkActionDto::Delete,
                deck_id: None,
                tag_id: None,
                tag_name: None,
                now_ms: 23_000,
            })
            .unwrap();
        assert_eq!(deleted.undo_action, Some(LibraryBulkActionDto::Restore));
        assert!(service.get_library(&request("")).unwrap().notes.is_empty());
        let mut trash = request("");
        trash.trash = LibraryTrashFilterDto::Deleted;
        assert_eq!(service.get_library(&trash).unwrap().notes.len(), 1);

        service
            .apply_library_bulk_action(&LibraryBulkRequest {
                source_ids: vec![source_id],
                action: LibraryBulkActionDto::Restore,
                deck_id: None,
                tag_id: None,
                tag_name: None,
                now_ms: 24_000,
            })
            .unwrap();
        let storage = Storage::open(&directory.path().join("collection.db")).unwrap();
        assert_eq!(
            storage.review_count(SAMPLE_CARD_ID).unwrap(),
            history_before
        );
        assert_eq!(
            storage.load_schedule(SAMPLE_CARD_ID).unwrap(),
            schedule_before
        );
        assert_eq!(storage.media_reference_usage("library-media").unwrap(), 1);
    }

    #[test]
    fn normalized_substring_search_stays_bounded_on_a_large_fixture() {
        let (directory, _service, _source_id) = service_with_search_fixture();
        let storage = Storage::open(&directory.path().join("collection.db")).unwrap();
        let stored = storage.library_notes().unwrap().remove(0);
        let deck = storage.get_deck(DEFAULT_DECK_ID).unwrap();
        let record = LibraryRecord { stored, deck };
        let records = vec![record; 10_000];
        let request = request("図書館");
        let normalized = meiki_text::normalize_for_search(&request.query);

        let started = Instant::now();
        let matches = records
            .iter()
            .filter(|record| library_matches(record, &request, &normalized))
            .count();

        assert_eq!(matches, records.len());
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "10,000-note search took {:?}",
            started.elapsed()
        );
    }
}
