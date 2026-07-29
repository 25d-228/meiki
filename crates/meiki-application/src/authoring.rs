use std::collections::{HashMap, HashSet};

use chrono::Utc;
use meiki_domain::{
    Annotation, Card, Cloze, Direction, LocalizedText, MatchingPolicy, ScheduleState,
    SegmentContent, SemanticSegment, SourceItem, StudySettingsOverride,
};
use meiki_storage::{
    CardRepository, DEFAULT_DECK_ID, DeckRepository, SourceNoteRepository, StoredSourceNote,
};
use meiki_text::GraphemeIndex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::{ApplicationError, ApplicationService, DirectionDto};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum AuthoringSegmentKindDto {
    Text,
    Cloze,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum MatchingPolicyDto {
    Strict,
    Forgiving,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct AnnotationDraftDto {
    pub id: String,
    pub label: String,
    pub value: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct AuthoringSegmentDto {
    pub id: String,
    pub ordinal: u32,
    pub kind: AuthoringSegmentKindDto,
    pub text: String,
    pub cloze_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct AuthoringClozeDto {
    pub id: String,
    pub card_id: String,
    pub answer: String,
    pub accepted_answers: Vec<String>,
    pub hint: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub matching_policy: Option<MatchingPolicyDto>,
    pub annotations: Vec<AnnotationDraftDto>,
    pub explanation_markdown: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct AuthoringDraftDto {
    pub source_id: String,
    pub deck_id: String,
    pub persisted: bool,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    pub deck_language_tag: Option<String>,
    pub deck_direction: DirectionDto,
    pub deck_matching_policy: MatchingPolicyDto,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub segments: Vec<AuthoringSegmentDto>,
    pub clozes: Vec<AuthoringClozeDto>,
    pub active_cloze_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct AuthoringPreviewDto {
    pub cloze_id: String,
    pub prompt: String,
    pub answer: String,
    pub language_tag: Option<String>,
    pub direction: DirectionDto,
    pub hint: String,
    pub annotations: Vec<AnnotationDraftDto>,
    pub explanation_markdown: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct MakeClozeRequest {
    pub draft: AuthoringDraftDto,
    pub segment_id: String,
    pub selection_start_utf16: u32,
    pub selection_end_utf16: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct RemoveClozeRequest {
    pub draft: AuthoringDraftDto,
    pub cloze_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
pub struct ReorderSegmentsRequest {
    pub draft: AuthoringDraftDto,
    pub segment_ids: Vec<String>,
}

impl ApplicationService {
    /// Starts a source-first authoring draft with stable aggregate identities.
    ///
    /// # Errors
    ///
    /// Returns an error when the default deck cannot be loaded.
    pub fn new_authoring_draft(&self) -> Result<AuthoringDraftDto, ApplicationError> {
        let now_ms = Utc::now().timestamp_millis();
        let storage = self.open_storage()?;
        let deck = storage.get_deck(DEFAULT_DECK_ID)?;
        Ok(AuthoringDraftDto {
            source_id: new_id(),
            deck_id: DEFAULT_DECK_ID.to_owned(),
            persisted: false,
            created_at_ms: now_ms,
            deck_language_tag: deck.language_tag,
            deck_direction: deck.direction.into(),
            deck_matching_policy: deck.matching_policy.into(),
            language_tag: None,
            direction: DirectionDto::Auto,
            segments: vec![AuthoringSegmentDto {
                id: new_id(),
                ordinal: 0,
                kind: AuthoringSegmentKindDto::Text,
                text: String::new(),
                cloze_id: None,
            }],
            clozes: Vec::new(),
            active_cloze_id: None,
        })
    }

    /// Turns a browser selection in a plain segment into a stable cloze.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection is empty, crosses a grapheme
    /// boundary, or does not belong to a plain-text segment.
    pub fn make_cloze(
        &self,
        request: MakeClozeRequest,
    ) -> Result<AuthoringDraftDto, ApplicationError> {
        let mut draft = request.draft;
        validate_draft_shape(&draft)?;
        let index = draft
            .segments
            .iter()
            .position(|segment| segment.id == request.segment_id)
            .ok_or_else(|| invalid("the selected segment no longer exists"))?;
        let segment = draft.segments.remove(index);
        if segment.kind != AuthoringSegmentKindDto::Text || segment.cloze_id.is_some() {
            return Err(invalid("only plain-text segments can become clozes"));
        }
        let start = usize::try_from(request.selection_start_utf16)
            .map_err(|_| invalid("the selection start is too large"))?;
        let end = usize::try_from(request.selection_end_utf16)
            .map_err(|_| invalid("the selection end is too large"))?;
        let split = GraphemeIndex::new(&segment.text).split_utf16(start..end)?;
        if split.selected.is_empty() {
            return Err(invalid("select at least one complete grapheme"));
        }

        let cloze_id = new_id();
        let has_before = !split.before.is_empty();
        let mut replacement = Vec::with_capacity(3);
        if has_before {
            replacement.push(text_segment(segment.id.clone(), split.before));
        }
        replacement.push(AuthoringSegmentDto {
            id: new_id(),
            ordinal: 0,
            kind: AuthoringSegmentKindDto::Cloze,
            text: split.selected.clone(),
            cloze_id: Some(cloze_id.clone()),
        });
        if !split.after.is_empty() {
            let id = if has_before { new_id() } else { segment.id };
            replacement.push(text_segment(id, split.after));
        }
        draft.segments.splice(index..index, replacement);
        draft.clozes.push(AuthoringClozeDto {
            id: cloze_id.clone(),
            card_id: new_id(),
            answer: split.selected,
            accepted_answers: Vec::new(),
            hint: String::new(),
            language_tag: draft.language_tag.clone(),
            direction: draft.direction,
            matching_policy: None,
            annotations: Vec::new(),
            explanation_markdown: String::new(),
        });
        draft.active_cloze_id = Some(cloze_id);
        renumber(&mut draft.segments)?;
        validate_draft_shape(&draft)?;
        Ok(draft)
    }

    /// Converts a cloze back to ordinary source text.
    ///
    /// # Errors
    ///
    /// Returns an error when the cloze is not part of the draft.
    pub fn remove_cloze(
        &self,
        request: RemoveClozeRequest,
    ) -> Result<AuthoringDraftDto, ApplicationError> {
        let mut draft = request.draft;
        validate_draft_shape(&draft)?;
        let index = draft
            .segments
            .iter()
            .position(|segment| segment.cloze_id.as_deref() == Some(&request.cloze_id))
            .ok_or_else(|| invalid("the cloze no longer exists"))?;
        draft.segments[index].kind = AuthoringSegmentKindDto::Text;
        draft.segments[index].cloze_id = None;
        draft.clozes.retain(|cloze| cloze.id != request.cloze_id);
        merge_adjacent_text(&mut draft.segments);
        draft.active_cloze_id = draft.clozes.first().map(|cloze| cloze.id.clone());
        renumber(&mut draft.segments)?;
        validate_draft_shape(&draft)?;
        Ok(draft)
    }

    /// Reorders complete semantic segments without detaching their metadata.
    ///
    /// # Errors
    ///
    /// Returns an error unless every existing segment identity occurs once.
    pub fn reorder_segments(
        &self,
        request: ReorderSegmentsRequest,
    ) -> Result<AuthoringDraftDto, ApplicationError> {
        let mut draft = request.draft;
        validate_draft_shape(&draft)?;
        if request.segment_ids.len() != draft.segments.len() {
            return Err(invalid("the segment order must include every segment"));
        }
        let mut by_id = draft
            .segments
            .drain(..)
            .map(|segment| (segment.id.clone(), segment))
            .collect::<HashMap<_, _>>();
        let mut reordered = Vec::with_capacity(request.segment_ids.len());
        for id in request.segment_ids {
            reordered.push(
                by_id
                    .remove(&id)
                    .ok_or_else(|| invalid("the segment order contains an unknown identity"))?,
            );
        }
        if !by_id.is_empty() {
            return Err(invalid("the segment order contains duplicate identities"));
        }
        draft.segments = reordered;
        renumber(&mut draft.segments)?;
        Ok(draft)
    }

    /// Builds one independent preview per cloze.
    ///
    /// # Errors
    ///
    /// Returns an error when draft identities are inconsistent.
    pub fn preview_authoring_draft(
        &self,
        draft: &AuthoringDraftDto,
    ) -> Result<Vec<AuthoringPreviewDto>, ApplicationError> {
        validate_draft_shape(draft)?;
        Ok(draft
            .clozes
            .iter()
            .map(|cloze| AuthoringPreviewDto {
                cloze_id: cloze.id.clone(),
                prompt: draft
                    .segments
                    .iter()
                    .map(|segment| {
                        if segment.cloze_id.as_deref() == Some(&cloze.id) {
                            "[…]".to_owned()
                        } else {
                            segment.text.clone()
                        }
                    })
                    .collect(),
                answer: cloze.answer.clone(),
                language_tag: cloze
                    .language_tag
                    .clone()
                    .or_else(|| draft.language_tag.clone())
                    .or_else(|| draft.deck_language_tag.clone()),
                direction: resolved_direction(
                    cloze.direction,
                    draft.direction,
                    draft.deck_direction,
                ),
                hint: cloze.hint.clone(),
                annotations: cloze.annotations.clone(),
                explanation_markdown: cloze.explanation_markdown.clone(),
            })
            .collect())
    }

    /// Saves the source note and ensures each cloze owns one schedulable card.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe Markdown, inconsistent identities, or
    /// persistence failures.
    pub fn save_authoring_draft(
        &self,
        draft: &AuthoringDraftDto,
    ) -> Result<AuthoringDraftDto, ApplicationError> {
        validate_for_save(draft)?;
        let now_ms = Utc::now().timestamp_millis();
        let note = stored_note(draft, now_ms);
        let mut storage = self.open_storage()?;

        if draft.persisted {
            let existing = storage.get_source_note(&draft.source_id)?;
            let requested = draft
                .clozes
                .iter()
                .map(|cloze| cloze.id.as_str())
                .collect::<HashSet<_>>();
            for cloze in existing.clozes {
                if !requested.contains(cloze.id.as_str()) {
                    let card = storage.get_card_for_cloze(&cloze.id)?;
                    storage.delete_card(&card.id)?;
                }
            }
            storage.update_source_note(&note)?;
        } else {
            storage.create_source_note(&note)?;
        }

        for cloze in &draft.clozes {
            match storage.get_card_for_cloze(&cloze.id) {
                Ok(mut card) => {
                    if card.id != cloze.card_id {
                        return Err(invalid("a cloze cannot change its card identity"));
                    }
                    card.content_version = card.content_version.saturating_add(1);
                    card.updated_at_ms = now_ms;
                    storage.update_card(&card)?;
                }
                Err(meiki_storage::StorageError::EntityNotFound { .. }) => {
                    let card = Card {
                        id: cloze.card_id.clone(),
                        cloze_id: cloze.id.clone(),
                        content_version: 0,
                        settings: StudySettingsOverride::default(),
                        created_at_ms: now_ms,
                        updated_at_ms: now_ms,
                    };
                    storage.create_card(
                        &card,
                        &ScheduleState {
                            card_id: card.id.clone(),
                            version: 0,
                            due_at_ms: now_ms,
                            interval_seconds: 0,
                            repetitions: 0,
                            last_review_event_id: None,
                        },
                    )?;
                }
                Err(error) => return Err(error.into()),
            }
        }

        let mut saved = draft.clone();
        saved.persisted = true;
        Ok(saved)
    }
}

fn validate_for_save(draft: &AuthoringDraftDto) -> Result<(), ApplicationError> {
    validate_draft_shape(draft)?;
    if draft.clozes.is_empty() {
        return Err(invalid("create at least one cloze before saving"));
    }
    if draft.segments.iter().all(|segment| segment.text.is_empty()) {
        return Err(invalid("source content cannot be empty"));
    }
    for cloze in &draft.clozes {
        validate_limited_markdown(&cloze.explanation_markdown)?;
        if cloze.answer.is_empty() {
            return Err(invalid("cloze answers cannot be empty"));
        }
        if cloze
            .accepted_answers
            .iter()
            .any(|answer| answer.trim().is_empty())
        {
            return Err(invalid("accepted answers cannot be empty"));
        }
        for annotation in &cloze.annotations {
            if annotation.label.trim().is_empty() || annotation.value.trim().is_empty() {
                return Err(invalid("annotations require both a label and a value"));
            }
        }
    }
    Ok(())
}

fn validate_draft_shape(draft: &AuthoringDraftDto) -> Result<(), ApplicationError> {
    if draft.segments.is_empty() {
        return Err(invalid("a draft requires at least one semantic segment"));
    }
    let clozes = draft
        .clozes
        .iter()
        .map(|cloze| (cloze.id.as_str(), cloze))
        .collect::<HashMap<_, _>>();
    if clozes.len() != draft.clozes.len() {
        return Err(invalid("cloze identities must be unique"));
    }
    let card_ids = draft
        .clozes
        .iter()
        .map(|cloze| cloze.card_id.as_str())
        .collect::<HashSet<_>>();
    if card_ids.len() != draft.clozes.len() {
        return Err(invalid("card identities must be unique"));
    }
    let annotation_count = draft
        .clozes
        .iter()
        .map(|cloze| cloze.annotations.len())
        .sum::<usize>();
    let annotation_ids = draft
        .clozes
        .iter()
        .flat_map(|cloze| cloze.annotations.iter())
        .map(|annotation| annotation.id.as_str())
        .collect::<HashSet<_>>();
    if annotation_ids.len() != annotation_count {
        return Err(invalid("annotation identities must be unique"));
    }
    let segment_ids = draft
        .segments
        .iter()
        .map(|segment| segment.id.as_str())
        .collect::<HashSet<_>>();
    if segment_ids.len() != draft.segments.len() {
        return Err(invalid("segment identities must be unique"));
    }
    let mut referenced = HashSet::new();
    for (index, segment) in draft.segments.iter().enumerate() {
        let ordinal = u32::try_from(index).map_err(|_| invalid("too many semantic segments"))?;
        if segment.ordinal != ordinal {
            return Err(invalid("segment ordinals must match their order"));
        }
        match segment.kind {
            AuthoringSegmentKindDto::Text if segment.cloze_id.is_none() => {}
            AuthoringSegmentKindDto::Cloze => {
                let cloze_id = segment
                    .cloze_id
                    .as_deref()
                    .ok_or_else(|| invalid("a cloze segment requires a cloze identity"))?;
                let cloze = clozes
                    .get(cloze_id)
                    .ok_or_else(|| invalid("a segment references a missing cloze"))?;
                if cloze.answer != segment.text || !referenced.insert(cloze_id) {
                    return Err(invalid(
                        "each cloze must preserve one matching surface segment",
                    ));
                }
            }
            AuthoringSegmentKindDto::Text => {
                return Err(invalid("plain-text segments cannot reference clozes"));
            }
        }
    }
    if referenced.len() != clozes.len() {
        return Err(invalid("every cloze requires one semantic segment"));
    }
    if let Some(active) = &draft.active_cloze_id {
        if !clozes.contains_key(active.as_str()) {
            return Err(invalid("the active cloze no longer exists"));
        }
    }
    Ok(())
}

fn validate_limited_markdown(value: &str) -> Result<(), ApplicationError> {
    let lower = value.to_lowercase();
    let forbidden = [
        "<",
        ">",
        "javascript:",
        "data:text/html",
        "onerror=",
        "onload=",
    ];
    if forbidden.iter().any(|token| lower.contains(token)) {
        return Err(invalid(
            "explanations support limited Markdown but not raw HTML or executable links",
        ));
    }
    Ok(())
}

fn stored_note(draft: &AuthoringDraftDto, now_ms: i64) -> StoredSourceNote {
    StoredSourceNote {
        source_item: SourceItem {
            id: draft.source_id.clone(),
            deck_id: draft.deck_id.clone(),
            segments: draft
                .segments
                .iter()
                .map(|segment| SemanticSegment {
                    id: segment.id.clone(),
                    ordinal: segment.ordinal,
                    content: match &segment.cloze_id {
                        Some(cloze_id) => SegmentContent::Cloze {
                            cloze_id: cloze_id.clone(),
                            text: segment.text.clone(),
                        },
                        None => SegmentContent::Text(segment.text.clone()),
                    },
                })
                .collect(),
            language_tag: draft.language_tag.clone(),
            direction: draft.direction.into(),
            tags: Vec::new(),
            annotations: Vec::new(),
            explanation: None,
            media: Vec::new(),
            created_at_ms: draft.created_at_ms,
            updated_at_ms: now_ms,
        },
        clozes: draft
            .clozes
            .iter()
            .map(|cloze| Cloze {
                id: cloze.id.clone(),
                source_item_id: draft.source_id.clone(),
                answer: cloze.answer.clone(),
                accepted_answers: cloze.accepted_answers.clone(),
                hint: localized(&cloze.hint, cloze.language_tag.clone(), cloze.direction),
                language_tag: cloze.language_tag.clone(),
                direction: cloze.direction.into(),
                matching_policy: cloze.matching_policy.map(Into::into),
                annotations: cloze
                    .annotations
                    .iter()
                    .map(|annotation| Annotation {
                        id: annotation.id.clone(),
                        label: annotation.label.clone(),
                        value: annotation.value.clone(),
                        language_tag: annotation.language_tag.clone(),
                        direction: annotation.direction.into(),
                    })
                    .collect(),
                explanation: localized(
                    &cloze.explanation_markdown,
                    cloze.language_tag.clone(),
                    cloze.direction,
                ),
                media: Vec::new(),
                created_at_ms: draft.created_at_ms,
                updated_at_ms: now_ms,
            })
            .collect(),
    }
}

fn localized(
    value: &str,
    language_tag: Option<String>,
    direction: DirectionDto,
) -> Option<LocalizedText> {
    (!value.is_empty()).then(|| LocalizedText {
        value: value.to_owned(),
        language_tag,
        direction: direction.into(),
    })
}

fn text_segment(id: String, text: String) -> AuthoringSegmentDto {
    AuthoringSegmentDto {
        id,
        ordinal: 0,
        kind: AuthoringSegmentKindDto::Text,
        text,
        cloze_id: None,
    }
}

fn merge_adjacent_text(segments: &mut Vec<AuthoringSegmentDto>) {
    let mut index = 0;
    while index + 1 < segments.len() {
        if segments[index].kind == AuthoringSegmentKindDto::Text
            && segments[index + 1].kind == AuthoringSegmentKindDto::Text
        {
            let next = segments.remove(index + 1);
            segments[index].text.push_str(&next.text);
        } else {
            index += 1;
        }
    }
}

fn renumber(segments: &mut [AuthoringSegmentDto]) -> Result<(), ApplicationError> {
    for (index, segment) in segments.iter_mut().enumerate() {
        segment.ordinal =
            u32::try_from(index).map_err(|_| invalid("too many semantic segments"))?;
    }
    Ok(())
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn invalid(message: &str) -> ApplicationError {
    ApplicationError::InvalidAuthoring(message.to_owned())
}

impl From<DirectionDto> for Direction {
    fn from(value: DirectionDto) -> Self {
        match value {
            DirectionDto::Auto => Self::Auto,
            DirectionDto::Ltr => Self::LeftToRight,
            DirectionDto::Rtl => Self::RightToLeft,
        }
    }
}

impl From<MatchingPolicyDto> for MatchingPolicy {
    fn from(value: MatchingPolicyDto) -> Self {
        match value {
            MatchingPolicyDto::Strict => Self::Strict,
            MatchingPolicyDto::Forgiving => Self::Forgiving,
        }
    }
}

impl From<MatchingPolicy> for MatchingPolicyDto {
    fn from(value: MatchingPolicy) -> Self {
        match value {
            MatchingPolicy::Strict => Self::Strict,
            MatchingPolicy::Forgiving => Self::Forgiving,
        }
    }
}

fn resolved_direction(
    cloze: DirectionDto,
    source: DirectionDto,
    deck: DirectionDto,
) -> DirectionDto {
    if cloze != DirectionDto::Auto {
        cloze
    } else if source != DirectionDto::Auto {
        source
    } else {
        deck
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::{CheckAnswerRequest, ComparisonResultDto};

    use super::{
        ApplicationService, AuthoringSegmentKindDto, MakeClozeRequest, MatchingPolicyDto,
        RemoveClozeRequest, ReorderSegmentsRequest,
    };

    fn service() -> (tempfile::TempDir, ApplicationService) {
        let directory = tempdir().unwrap();
        let service = ApplicationService::new(directory.path().join("collection.db"));
        (directory, service)
    }

    #[test]
    fn creates_multiple_stable_clozes_and_independent_previews() {
        let (_directory, service) = service();
        let mut draft = service.new_authoring_draft().unwrap();
        draft.segments[0].text = "日曜日は図書館に行きます".into();
        let first = service
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id: String::new(),
                selection_start_utf16: 0,
                selection_end_utf16: 0,
            })
            .unwrap_err();
        assert!(first.to_string().contains("selected segment"));

        let mut draft = service.new_authoring_draft().unwrap();
        draft.segments[0].text = "日曜日は図書館に行きます".into();
        let segment_id = draft.segments[0].id.clone();
        let draft = service
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id,
                selection_start_utf16: 4,
                selection_end_utf16: 7,
            })
            .unwrap();
        let tail_id = draft.segments.last().unwrap().id.clone();
        let tail_len = draft.segments.last().unwrap().text.encode_utf16().count();
        let draft = service
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id: tail_id,
                selection_start_utf16: u32::try_from(tail_len - 4).unwrap(),
                selection_end_utf16: u32::try_from(tail_len).unwrap(),
            })
            .unwrap();
        let previews = service.preview_authoring_draft(&draft).unwrap();
        assert_eq!(previews.len(), 2);
        assert_ne!(previews[0].prompt, previews[1].prompt);
        assert!(previews[0].prompt.contains("[…]"));
        assert!(previews[1].prompt.contains("[…]"));
    }

    #[test]
    fn rejects_selection_inside_a_grapheme_and_round_trips_removal() {
        let (_directory, service) = service();
        let mut draft = service.new_authoring_draft().unwrap();
        draft.segments[0].text = "A👨‍👩‍👧‍👦B".into();
        let segment_id = draft.segments[0].id.clone();
        let error = service
            .make_cloze(MakeClozeRequest {
                draft: draft.clone(),
                segment_id: segment_id.clone(),
                selection_start_utf16: 2,
                selection_end_utf16: 3,
            })
            .unwrap_err();
        assert!(error.to_string().contains("grapheme"));

        let family_end = 1 + "👨‍👩‍👧‍👦".encode_utf16().count();
        let draft = service
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id,
                selection_start_utf16: 1,
                selection_end_utf16: u32::try_from(family_end).unwrap(),
            })
            .unwrap();
        let cloze_id = draft.clozes[0].id.clone();
        let draft = service
            .remove_cloze(RemoveClozeRequest { draft, cloze_id })
            .unwrap();
        assert_eq!(draft.segments.len(), 1);
        assert_eq!(draft.segments[0].text, "A👨‍👩‍👧‍👦B");
        assert_eq!(draft.segments[0].kind, AuthoringSegmentKindDto::Text);
    }

    #[test]
    fn reorders_segments_and_persists_one_card_per_cloze() {
        let (_directory, service) = service();
        let mut draft = service.new_authoring_draft().unwrap();
        draft.segments[0].text = "abc".into();
        let segment_id = draft.segments[0].id.clone();
        let draft = service
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id,
                selection_start_utf16: 1,
                selection_end_utf16: 2,
            })
            .unwrap();
        let reversed = draft
            .segments
            .iter()
            .rev()
            .map(|segment| segment.id.clone())
            .collect();
        let draft = service
            .reorder_segments(ReorderSegmentsRequest {
                draft,
                segment_ids: reversed,
            })
            .unwrap();
        assert_eq!(draft.segments[0].text, "c");

        let saved = service.save_authoring_draft(&draft).unwrap();
        assert!(saved.persisted);
        let card = service.get_study_card(&saved.clozes[0].card_id).unwrap();
        assert_eq!(card.prompt, "c[…]a");
    }

    #[test]
    fn rejects_executable_html_but_keeps_limited_markdown() {
        let (_directory, service) = service();
        let mut draft = service.new_authoring_draft().unwrap();
        draft.segments[0].text = "café".into();
        let segment_id = draft.segments[0].id.clone();
        let mut draft = service
            .make_cloze(MakeClozeRequest {
                draft,
                segment_id,
                selection_start_utf16: 0,
                selection_end_utf16: 4,
            })
            .unwrap();
        draft.clozes[0].explanation_markdown = "**remember** this".into();
        draft.clozes[0].matching_policy = Some(MatchingPolicyDto::Forgiving);
        let saved = service.save_authoring_draft(&draft).unwrap();
        let card = service.get_study_card(&saved.clozes[0].card_id).unwrap();
        let reveal = service
            .check_answer(&CheckAnswerRequest {
                card_id: card.card_id,
                card_content_version: card.card_content_version,
                schedule_version: card.schedule_version,
                raw_response: "CAFE".into(),
            })
            .unwrap();
        assert_eq!(reveal.comparison, ComparisonResultDto::Exact);
        draft.persisted = true;
        draft.clozes[0].explanation_markdown = "<script>alert(1)</script>".into();
        assert!(service.save_authoring_draft(&draft).is_err());
    }
}
