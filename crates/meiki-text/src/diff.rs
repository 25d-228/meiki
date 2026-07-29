use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiffKind {
    Equal,
    Delete,
    Insert,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffSegment {
    pub kind: DiffKind,
    pub text: String,
}

pub fn grapheme_diff(expected: &str, response: &str) -> Vec<DiffSegment> {
    let expected: Vec<&str> = expected.graphemes(true).collect();
    let response: Vec<&str> = response.graphemes(true).collect();
    let mut common = vec![vec![0_usize; response.len() + 1]; expected.len() + 1];

    for expected_index in (0..expected.len()).rev() {
        for response_index in (0..response.len()).rev() {
            common[expected_index][response_index] =
                if expected[expected_index] == response[response_index] {
                    common[expected_index + 1][response_index + 1] + 1
                } else {
                    common[expected_index + 1][response_index]
                        .max(common[expected_index][response_index + 1])
                };
        }
    }

    let mut difference = Vec::new();
    let (mut expected_index, mut response_index) = (0, 0);
    while expected_index < expected.len() || response_index < response.len() {
        if expected_index < expected.len()
            && response_index < response.len()
            && expected[expected_index] == response[response_index]
        {
            push_segment(&mut difference, DiffKind::Equal, expected[expected_index]);
            expected_index += 1;
            response_index += 1;
        } else if expected_index < expected.len()
            && (response_index == response.len()
                || common[expected_index + 1][response_index]
                    >= common[expected_index][response_index + 1])
        {
            push_segment(&mut difference, DiffKind::Delete, expected[expected_index]);
            expected_index += 1;
        } else {
            push_segment(&mut difference, DiffKind::Insert, response[response_index]);
            response_index += 1;
        }
    }
    difference
}

pub fn grapheme_distance(left: &str, right: &str) -> usize {
    let left: Vec<&str> = left.graphemes(true).collect();
    let right: Vec<&str> = right.graphemes(true).collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (left_index, left_grapheme) in left.iter().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_grapheme) in right.iter().enumerate() {
            let substitution = previous[right_index] + usize::from(left_grapheme != right_grapheme);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(substitution);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

fn push_segment(segments: &mut Vec<DiffSegment>, kind: DiffKind, grapheme: &str) {
    if let Some(last) = segments.last_mut() {
        if last.kind == kind {
            last.text.push_str(grapheme);
            return;
        }
    }
    segments.push(DiffSegment {
        kind,
        text: grapheme.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::{DiffKind, DiffSegment, grapheme_diff, grapheme_distance};

    #[test]
    fn diff_keeps_combining_sequences_and_emoji_together() {
        assert_eq!(
            grapheme_diff("नमस्ते 👨‍👩‍👧", "नमस्ते 👨‍👩‍👦"),
            vec![
                DiffSegment {
                    kind: DiffKind::Equal,
                    text: "नमस्ते ".to_owned(),
                },
                DiffSegment {
                    kind: DiffKind::Delete,
                    text: "👨‍👩‍👧".to_owned(),
                },
                DiffSegment {
                    kind: DiffKind::Insert,
                    text: "👨‍👩‍👦".to_owned(),
                },
            ]
        );
        assert_eq!(grapheme_distance("👨‍👩‍👧", "👨‍👩‍👦"), 1);
    }
}
