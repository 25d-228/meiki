use meiki_domain::{ComparisonResult, Direction};
use meiki_text::{
    BidiRenderContract, CaseSensitivity, ComparisonOptions, DiacriticSensitivity, DiffKind,
    GraphemeIndex, GraphemeRange, PunctuationSensitivity, TextBoundaryError, WhitespaceSensitivity,
    WidthSensitivity, compare_answer, compare_answer_with_options, grapheme_diff,
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
    ("\u{301}\u{327}", Direction::Auto),
    ("✈️ 🇯🇵 👩🏽‍💻", Direction::Auto),
    ("العربية ١٢٣، (ABC-42) שלום", Direction::RightToLeft),
    ("क्\u{200d}षि क्ष", Direction::LeftToRight),
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

#[test]
fn hostile_unicode_boundaries_round_trip_without_splitting_a_grapheme() {
    for &(fixture, _) in FIXTURES {
        let original = fixture.as_bytes().to_vec();
        let index = GraphemeIndex::new(fixture);
        for utf16_offset in 0..=fixture.encode_utf16().count() {
            match index.grapheme_index_at_utf16(utf16_offset) {
                Ok(grapheme) => {
                    assert_eq!(index.utf16_index(grapheme), Ok(utf16_offset));
                }
                Err(error) => assert_eq!(error, TextBoundaryError::SplitsGrapheme),
            }
        }
        for byte_offset in 0..=fixture.len() {
            match index.grapheme_index_at_byte(byte_offset) {
                Ok(grapheme) => assert_eq!(index.byte_index(grapheme), Ok(byte_offset)),
                Err(error) => assert_eq!(error, TextBoundaryError::SplitsGrapheme),
            }
        }
        let split = index.split(GraphemeRange::new(0, index.len())).unwrap();
        assert_eq!(split.selected.as_bytes(), original);
        assert_eq!(fixture.as_bytes(), original);
    }

    let surrogate_pair = GraphemeIndex::new("😀");
    assert_eq!(
        surrogate_pair.grapheme_index_at_utf16(1),
        Err(TextBoundaryError::SplitsGrapheme)
    );
    assert_eq!(
        surrogate_pair.split_utf16(0..1),
        Err(TextBoundaryError::SplitsGrapheme)
    );
}

#[test]
fn strict_and_forgiving_policies_apply_only_their_documented_transformations() {
    let expected = "Café！ au  lait";
    let response = "ＣＡＦＥ au lait";
    let strict = compare_answer(expected, &[], response);
    assert_eq!(strict.result, ComparisonResult::Incorrect);
    assert_eq!(strict.normalized_expected, expected);
    assert_eq!(strict.normalized_response, response);

    let forgiving = ComparisonOptions {
        case: CaseSensitivity::UnicodeLowercase,
        diacritics: DiacriticSensitivity::Ignore,
        punctuation: PunctuationSensitivity::Ignore,
        whitespace: WhitespaceSensitivity::Collapse,
        width: WidthSensitivity::FoldCompatibility,
        ..ComparisonOptions::default()
    };
    let relaxed = compare_answer_with_options(expected, &[], response, &forgiving);
    assert_eq!(relaxed.result, ComparisonResult::Exact);
    assert_eq!(relaxed.normalized_expected, "cafe au lait");
    assert_eq!(relaxed.normalized_response, "cafe au lait");

    let bidi = "العربية ١٢٣، (ABC-42) שלום";
    assert_eq!(normalize_for_default_comparison(bidi), bidi);
    assert_eq!(normalize_for_search(bidi), bidi.to_lowercase());
}

#[test]
fn long_mixed_script_content_keeps_storage_text_and_derived_values_separate() {
    let unit = "Cafe\u{301}・東京・العربية ١٢٣・क्\u{200d}षि・✈️・🇯🇵\n";
    let raw = unit.repeat(4_096);
    let bytes = raw.as_bytes().to_vec();
    let search_key = normalize_for_search(&raw);
    assert!(search_key.contains("café"));
    assert!(search_contains(&raw, "東京・العربية"));
    assert_eq!(raw.as_bytes(), bytes);

    // The diff implementation is intentionally quadratic. Exercise every hostile
    // boundary on a bounded slice while keeping the storage/search payload large.
    let diff_input = unit.repeat(32);
    let difference = grapheme_diff(&diff_input, &diff_input);
    assert_eq!(difference.len(), 1);
    assert_eq!(difference[0].kind, DiffKind::Equal);
    assert_eq!(difference[0].text, diff_input);
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
