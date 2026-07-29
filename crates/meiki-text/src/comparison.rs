use meiki_domain::ComparisonResult;
use unicode_general_category::{GeneralCategory, get_general_category};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use unicode_segmentation::UnicodeSegmentation;

use crate::{DiffSegment, grapheme_diff, grapheme_distance};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CaseSensitivity {
    Sensitive,
    UnicodeLowercase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiacriticSensitivity {
    Significant,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PunctuationSensitivity {
    Significant,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WhitespaceSensitivity {
    Exact,
    Collapse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WidthSensitivity {
    Significant,
    FoldCompatibility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NearMatchOptions {
    pub enabled: bool,
    pub minimum_graphemes: usize,
    pub maximum_distance: usize,
    pub maximum_ratio_basis_points: usize,
}

impl Default for NearMatchOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            minimum_graphemes: 5,
            maximum_distance: 2,
            maximum_ratio_basis_points: 2_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComparisonOptions {
    pub trim_outer_whitespace: bool,
    pub case: CaseSensitivity,
    pub diacritics: DiacriticSensitivity,
    pub punctuation: PunctuationSensitivity,
    pub whitespace: WhitespaceSensitivity,
    pub width: WidthSensitivity,
    pub near_match: NearMatchOptions,
}

impl Default for ComparisonOptions {
    fn default() -> Self {
        Self {
            trim_outer_whitespace: true,
            case: CaseSensitivity::Sensitive,
            diacritics: DiacriticSensitivity::Significant,
            punctuation: PunctuationSensitivity::Significant,
            whitespace: WhitespaceSensitivity::Exact,
            width: WidthSensitivity::Significant,
            near_match: NearMatchOptions::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comparison {
    pub result: ComparisonResult,
    pub normalized_expected: String,
    pub normalized_response: String,
    pub reference_accepted_answer: Option<usize>,
    pub difference: Vec<DiffSegment>,
}

pub fn normalize_for_default_comparison(value: &str) -> String {
    normalize_for_comparison(value, &ComparisonOptions::default())
}

pub fn normalize_for_comparison(value: &str, options: &ComparisonOptions) -> String {
    let mut normalized: String = match options.width {
        WidthSensitivity::Significant => value.nfc().collect(),
        WidthSensitivity::FoldCompatibility => value.nfkc().collect(),
    };

    if options.diacritics == DiacriticSensitivity::Ignore {
        normalized = normalized
            .nfd()
            .filter(|character| !is_combining_mark(*character))
            .collect();
    }
    if options.case == CaseSensitivity::UnicodeLowercase {
        normalized = normalized.chars().flat_map(char::to_lowercase).collect();
    }
    if options.punctuation == PunctuationSensitivity::Ignore {
        normalized.retain(|character| !is_punctuation(character));
    }
    if options.whitespace == WhitespaceSensitivity::Collapse {
        normalized = collapse_whitespace(&normalized);
    }
    if options.trim_outer_whitespace {
        normalized = normalized.trim().to_owned();
    }

    normalized.nfc().collect()
}

pub fn compare_answer(expected: &str, accepted: &[String], response: &str) -> Comparison {
    compare_answer_with_options(expected, accepted, response, &ComparisonOptions::default())
}

pub fn compare_answer_with_options(
    expected: &str,
    accepted: &[String],
    response: &str,
    options: &ComparisonOptions,
) -> Comparison {
    let normalized_expected = normalize_for_comparison(expected, options);
    let normalized_response = normalize_for_comparison(response, options);
    let normalized_accepted: Vec<String> = accepted
        .iter()
        .map(|candidate| normalize_for_comparison(candidate, options))
        .collect();

    let (result, reference_accepted_answer, reference) = if normalized_response.is_empty() {
        (ComparisonResult::Empty, None, normalized_expected.as_str())
    } else if normalized_response == normalized_expected {
        (ComparisonResult::Exact, None, normalized_expected.as_str())
    } else if let Some(index) = normalized_accepted
        .iter()
        .position(|candidate| *candidate == normalized_response)
    {
        (
            ComparisonResult::AcceptedVariant,
            Some(index),
            normalized_accepted[index].as_str(),
        )
    } else {
        let (closest_accepted_answer, closest_reference, distance) = closest_reference(
            &normalized_expected,
            &normalized_accepted,
            &normalized_response,
        );
        let result = if qualifies_as_near_match(
            closest_reference,
            &normalized_response,
            distance,
            options.near_match,
        ) {
            ComparisonResult::NearMatch
        } else {
            ComparisonResult::Incorrect
        };
        if result == ComparisonResult::NearMatch {
            (result, closest_accepted_answer, closest_reference)
        } else {
            (result, None, normalized_expected.as_str())
        }
    };

    let difference = grapheme_diff(reference, &normalized_response);
    Comparison {
        result,
        normalized_expected,
        normalized_response,
        reference_accepted_answer,
        difference,
    }
}

fn is_punctuation(character: char) -> bool {
    matches!(
        get_general_category(character),
        GeneralCategory::ClosePunctuation
            | GeneralCategory::ConnectorPunctuation
            | GeneralCategory::DashPunctuation
            | GeneralCategory::FinalPunctuation
            | GeneralCategory::InitialPunctuation
            | GeneralCategory::OpenPunctuation
            | GeneralCategory::OtherPunctuation
    )
}

fn collapse_whitespace(value: &str) -> String {
    let mut collapsed = String::with_capacity(value.len());
    let mut previous_was_whitespace = false;
    for character in value.chars() {
        if character.is_whitespace() {
            if !previous_was_whitespace {
                collapsed.push(' ');
            }
            previous_was_whitespace = true;
        } else {
            collapsed.push(character);
            previous_was_whitespace = false;
        }
    }
    collapsed
}

fn closest_reference<'a>(
    expected: &'a str,
    accepted: &'a [String],
    response: &str,
) -> (Option<usize>, &'a str, usize) {
    let mut best = (None, expected, grapheme_distance(expected, response));
    for (index, candidate) in accepted.iter().enumerate() {
        let distance = grapheme_distance(candidate, response);
        if distance < best.2 {
            best = (Some(index), candidate, distance);
        }
    }
    best
}

fn qualifies_as_near_match(
    expected: &str,
    response: &str,
    distance: usize,
    options: NearMatchOptions,
) -> bool {
    if !options.enabled || distance == 0 || distance > options.maximum_distance {
        return false;
    }
    let longest = expected
        .graphemes(true)
        .count()
        .max(response.graphemes(true).count());
    longest >= options.minimum_graphemes
        && distance.saturating_mul(10_000)
            <= longest.saturating_mul(options.maximum_ratio_basis_points)
}

#[cfg(test)]
mod tests {
    use meiki_domain::ComparisonResult;

    use super::{
        CaseSensitivity, ComparisonOptions, DiacriticSensitivity, PunctuationSensitivity,
        WhitespaceSensitivity, WidthSensitivity, compare_answer, compare_answer_with_options,
        normalize_for_default_comparison,
    };

    #[test]
    fn default_comparison_uses_nfc_and_outer_trim_only() {
        assert_eq!(normalize_for_default_comparison("  e\u{301}  "), "\u{e9}");
        assert_eq!(
            compare_answer("Café", &[], " cafe ").result,
            ComparisonResult::Incorrect
        );
        assert_eq!(
            compare_answer("Ａ", &[], "A").result,
            ComparisonResult::Incorrect
        );
    }

    #[test]
    fn accepted_variants_are_explicit() {
        let accepted = vec!["ゆきます".to_owned()];
        let comparison = compare_answer("行きます", &accepted, "ゆきます");
        assert_eq!(comparison.result, ComparisonResult::AcceptedVariant);
        assert_eq!(comparison.reference_accepted_answer, Some(0));
    }

    #[test]
    fn empty_response_is_distinct() {
        assert_eq!(
            compare_answer("行きます", &[], " \n").result,
            ComparisonResult::Empty
        );
    }

    #[test]
    fn near_matches_are_conservative_feedback() {
        assert_eq!(
            compare_answer("bibliothèque", &[], "bibliotheque").result,
            ComparisonResult::NearMatch
        );
        assert_eq!(
            compare_answer("کتاب", &[], "کباب").result,
            ComparisonResult::Incorrect
        );
    }

    #[test]
    fn explicit_rules_can_relax_individual_dimensions() {
        let options = ComparisonOptions {
            case: CaseSensitivity::UnicodeLowercase,
            diacritics: DiacriticSensitivity::Ignore,
            punctuation: PunctuationSensitivity::Ignore,
            whitespace: WhitespaceSensitivity::Collapse,
            width: WidthSensitivity::FoldCompatibility,
            ..ComparisonOptions::default()
        };
        assert_eq!(
            compare_answer_with_options("  Café！ au  lait ", &[], "ＣＡＦＥ au lait", &options)
                .result,
            ComparisonResult::Exact
        );
    }
}
