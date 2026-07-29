use std::{fmt, ops::Range};

use meiki_domain::{SegmentContent, SemanticSegment};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphemeRange {
    pub start: usize,
    pub end: usize,
}

impl GraphemeRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextSplit {
    pub before: String,
    pub selected: String,
    pub after: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPosition {
    pub segment_id: String,
    pub segment_ordinal: u32,
    pub grapheme_offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextBoundaryError {
    OutOfBounds,
    SplitsGrapheme,
    ReversedRange,
    NoSegments,
}

impl fmt::Display for TextBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds => formatter.write_str("text position is out of bounds"),
            Self::SplitsGrapheme => {
                formatter.write_str("text position splits an extended grapheme cluster")
            }
            Self::ReversedRange => formatter.write_str("text range end precedes its start"),
            Self::NoSegments => formatter.write_str("source contains no semantic segments"),
        }
    }
}

impl std::error::Error for TextBoundaryError {}

#[derive(Clone, Debug)]
pub struct GraphemeIndex<'a> {
    value: &'a str,
    byte_boundaries: Vec<usize>,
    utf16_boundaries: Vec<usize>,
}

impl<'a> GraphemeIndex<'a> {
    pub fn new(value: &'a str) -> Self {
        let mut byte_boundaries: Vec<usize> = value
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .collect();
        byte_boundaries.push(value.len());

        let mut utf16_boundaries = Vec::with_capacity(byte_boundaries.len());
        utf16_boundaries.push(0);
        let mut utf16_offset = 0;
        for grapheme in value.graphemes(true) {
            utf16_offset += grapheme.encode_utf16().count();
            utf16_boundaries.push(utf16_offset);
        }

        Self {
            value,
            byte_boundaries,
            utf16_boundaries,
        }
    }

    pub fn len(&self) -> usize {
        self.byte_boundaries.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the UTF-8 byte boundary for a grapheme position.
    ///
    /// # Errors
    ///
    /// Returns [`TextBoundaryError::OutOfBounds`] when the position exceeds
    /// the text's grapheme count.
    pub fn byte_index(&self, grapheme_index: usize) -> Result<usize, TextBoundaryError> {
        self.byte_boundaries
            .get(grapheme_index)
            .copied()
            .ok_or(TextBoundaryError::OutOfBounds)
    }

    /// Returns the browser UTF-16 boundary for a grapheme position.
    ///
    /// # Errors
    ///
    /// Returns [`TextBoundaryError::OutOfBounds`] when the position exceeds
    /// the text's grapheme count.
    pub fn utf16_index(&self, grapheme_index: usize) -> Result<usize, TextBoundaryError> {
        self.utf16_boundaries
            .get(grapheme_index)
            .copied()
            .ok_or(TextBoundaryError::OutOfBounds)
    }

    /// Resolves an exact UTF-8 byte boundary to its grapheme position.
    ///
    /// # Errors
    ///
    /// Returns an error when the byte offset is outside the text or falls
    /// inside a grapheme.
    pub fn grapheme_index_at_byte(&self, byte_index: usize) -> Result<usize, TextBoundaryError> {
        exact_boundary(&self.byte_boundaries, byte_index)
    }

    /// Resolves an exact browser UTF-16 boundary to its grapheme position.
    ///
    /// # Errors
    ///
    /// Returns an error when the code-unit offset is outside the text or falls
    /// inside a grapheme.
    pub fn grapheme_index_at_utf16(&self, utf16_index: usize) -> Result<usize, TextBoundaryError> {
        exact_boundary(&self.utf16_boundaries, utf16_index)
    }

    /// Splits the source at a grapheme-aligned range.
    ///
    /// # Errors
    ///
    /// Returns an error for a reversed or out-of-bounds range.
    pub fn split(&self, range: GraphemeRange) -> Result<TextSplit, TextBoundaryError> {
        if range.end < range.start {
            return Err(TextBoundaryError::ReversedRange);
        }
        let start = self.byte_index(range.start)?;
        let end = self.byte_index(range.end)?;
        Ok(TextSplit {
            before: self.value[..start].to_owned(),
            selected: self.value[start..end].to_owned(),
            after: self.value[end..].to_owned(),
        })
    }

    /// Splits the source at browser UTF-16 offsets.
    ///
    /// # Errors
    ///
    /// Returns an error when the range is reversed, outside the text, or
    /// splits a grapheme.
    pub fn split_utf16(&self, range: Range<usize>) -> Result<TextSplit, TextBoundaryError> {
        if range.end < range.start {
            return Err(TextBoundaryError::ReversedRange);
        }
        self.split(GraphemeRange {
            start: self.grapheme_index_at_utf16(range.start)?,
            end: self.grapheme_index_at_utf16(range.end)?,
        })
    }
}

/// Resolves a browser UTF-16 cursor to stable semantic-segment identity.
///
/// # Errors
///
/// Returns an error when no segments exist or the cursor is outside the source
/// or splits a grapheme.
pub fn semantic_position_from_utf16(
    segments: &[SemanticSegment],
    utf16_index: usize,
) -> Result<SemanticPosition, TextBoundaryError> {
    if segments.is_empty() {
        return Err(TextBoundaryError::NoSegments);
    }
    let rendered: String = segments.iter().map(segment_text).collect();
    let absolute_grapheme = GraphemeIndex::new(&rendered).grapheme_index_at_utf16(utf16_index)?;
    locate_semantic_position(segments, absolute_grapheme)
}

fn locate_semantic_position(
    segments: &[SemanticSegment],
    absolute_grapheme: usize,
) -> Result<SemanticPosition, TextBoundaryError> {
    let total = segments
        .iter()
        .map(|segment| segment_text(segment).graphemes(true).count())
        .sum::<usize>();
    if absolute_grapheme > total {
        return Err(TextBoundaryError::OutOfBounds);
    }

    let mut preceding = 0;
    for (index, segment) in segments.iter().enumerate() {
        let length = segment_text(segment).graphemes(true).count();
        let end = preceding + length;
        if absolute_grapheme < end || (index == segments.len() - 1 && absolute_grapheme == end) {
            return Ok(SemanticPosition {
                segment_id: segment.id.clone(),
                segment_ordinal: segment.ordinal,
                grapheme_offset: absolute_grapheme - preceding,
            });
        }
        preceding = end;
    }
    Err(TextBoundaryError::OutOfBounds)
}

fn segment_text(segment: &SemanticSegment) -> &str {
    match &segment.content {
        SegmentContent::Text(text) | SegmentContent::Cloze { text, .. } => text,
    }
}

fn exact_boundary(boundaries: &[usize], index: usize) -> Result<usize, TextBoundaryError> {
    if index > boundaries.last().copied().unwrap_or_default() {
        return Err(TextBoundaryError::OutOfBounds);
    }
    boundaries
        .binary_search(&index)
        .map_err(|_| TextBoundaryError::SplitsGrapheme)
}

#[cfg(test)]
mod tests {
    use meiki_domain::{SegmentContent, SemanticSegment};

    use super::{
        GraphemeIndex, GraphemeRange, SemanticPosition, TextBoundaryError,
        semantic_position_from_utf16,
    };

    #[test]
    fn browser_offsets_cannot_split_graphemes() {
        let text = "A👨‍👩‍👧नमस्ते";
        let index = GraphemeIndex::new(text);
        assert_eq!(index.len(), 5);
        assert_eq!(index.grapheme_index_at_utf16(1), Ok(1));
        assert_eq!(
            index.grapheme_index_at_utf16(2),
            Err(TextBoundaryError::SplitsGrapheme)
        );
        let family_end = index.utf16_index(2).unwrap();
        assert_eq!(index.split_utf16(1..family_end).unwrap().selected, "👨‍👩‍👧");
        assert_eq!(
            index.split(GraphemeRange::new(1, 2)).unwrap().selected,
            "👨‍👩‍👧"
        );
    }

    #[test]
    fn editor_positions_resolve_to_stable_segment_identity() {
        let segments = vec![
            SemanticSegment {
                id: "prefix".to_owned(),
                ordinal: 0,
                content: SegmentContent::Text("日曜日は".to_owned()),
            },
            SemanticSegment {
                id: "answer-segment".to_owned(),
                ordinal: 1,
                content: SegmentContent::Cloze {
                    cloze_id: "cloze-1".to_owned(),
                    text: "図書館".to_owned(),
                },
            },
        ];
        assert_eq!(
            semantic_position_from_utf16(&segments, 4),
            Ok(SemanticPosition {
                segment_id: "answer-segment".to_owned(),
                segment_ordinal: 1,
                grapheme_offset: 0,
            })
        );
    }
}
