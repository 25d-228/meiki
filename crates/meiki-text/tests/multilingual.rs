use meiki_domain::{ComparisonResult, Direction};
use meiki_text::{
    BidiRenderContract, DiffKind, GraphemeIndex, GraphemeRange, compare_answer, grapheme_diff,
    grapheme_distance, normalize_for_default_comparison, normalize_for_search, search_contains,
};
use proptest::prelude::*;
use unicode_segmentation::UnicodeSegmentation;

const FIXTURES: &[(&str, Direction)] = &[
    ("日曜日は図書館に行きます", Direction::Auto),
    ("أنا أقرأ كتابًا", Direction::RightToLeft),
    ("אני קורא ספר", Direction::RightToLeft),
    ("मैं पुस्तक पढ़ता हूँ", Direction::LeftToRight),
    ("Crème brûlée", Direction::LeftToRight),
    ("漢字仮名交じり文", Direction::Auto),
    ("Meeting الساعة 三時", Direction::Auto),
    ("👨‍👩‍👧‍👦 👍🏽 e\u{301}", Direction::Auto),
];

#[test]
fn required_scripts_round_trip_through_grapheme_and_bidi_contracts() {
    for &(fixture, direction) in FIXTURES {
        let index = GraphemeIndex::new(fixture);
        let split = index.split(GraphemeRange::new(0, index.len())).unwrap();
        assert_eq!(split.selected, fixture);
        assert_eq!(
            BidiRenderContract::new(fixture, direction).isolated_text,
            format!(
                "{}{fixture}\u{2069}",
                match direction {
                    Direction::Auto => '\u{2068}',
                    Direction::LeftToRight => '\u{2066}',
                    Direction::RightToLeft => '\u{2067}',
                }
            )
        );
    }
}

#[test]
fn canonical_equivalence_preserves_raw_input_but_compares_exactly() {
    let raw = " Cafe\u{301} ";
    let comparison = compare_answer("Café", &[], raw);
    assert_eq!(raw, " Cafe\u{301} ");
    assert_eq!(comparison.result, ComparisonResult::Exact);
    assert_eq!(comparison.normalized_response, "Café");
}

#[test]
fn substring_search_works_without_whitespace_or_language_detection() {
    assert!(search_contains("日曜日は図書館に行きます", "館に"));
    assert!(search_contains("Meeting الساعة 三時", "الس"));
    assert!(search_contains("Crème brûlée", "ＢＲÛＬÉＥ"));
}

proptest! {
    #[test]
    fn normalization_is_idempotent(value in unicode_string()) {
        let once = normalize_for_default_comparison(&value);
        prop_assert_eq!(normalize_for_default_comparison(&once), once);
    }

    #[test]
    fn search_keys_are_idempotent(value in unicode_string()) {
        let once = normalize_for_search(&value);
        prop_assert_eq!(normalize_for_search(&once), once);
    }

    #[test]
    fn grapheme_boundaries_round_trip(value in unicode_string()) {
        let index = GraphemeIndex::new(&value);
        prop_assert_eq!(index.len(), value.graphemes(true).count());
        for grapheme_index in 0..=index.len() {
            let byte = index.byte_index(grapheme_index).unwrap();
            let utf16 = index.utf16_index(grapheme_index).unwrap();
            prop_assert_eq!(index.grapheme_index_at_byte(byte), Ok(grapheme_index));
            prop_assert_eq!(index.grapheme_index_at_utf16(utf16), Ok(grapheme_index));
        }
        let split = index.split(GraphemeRange::new(0, index.len())).unwrap();
        prop_assert_eq!(split.selected, value);
    }

    #[test]
    fn diff_never_loses_or_splits_input(
        expected in unicode_string(),
        response in unicode_string(),
    ) {
        let difference = grapheme_diff(&expected, &response);
        let rebuilt_expected: String = difference
            .iter()
            .filter(|segment| segment.kind != DiffKind::Insert)
            .map(|segment| segment.text.as_str())
            .collect();
        let rebuilt_response: String = difference
            .iter()
            .filter(|segment| segment.kind != DiffKind::Delete)
            .map(|segment| segment.text.as_str())
            .collect();
        prop_assert_eq!(rebuilt_expected, expected);
        prop_assert_eq!(rebuilt_response, response);
    }

    #[test]
    fn grapheme_distance_is_symmetric(
        left in unicode_string(),
        right in unicode_string(),
    ) {
        prop_assert_eq!(
            grapheme_distance(&left, &right),
            grapheme_distance(&right, &left)
        );
        prop_assert_eq!(grapheme_distance(&left, &left), 0);
    }
}

fn unicode_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(any::<char>(), 0..48)
        .prop_map(|characters| characters.into_iter().collect())
}
