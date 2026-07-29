use meiki_domain::Direction;

const LEFT_TO_RIGHT_ISOLATE: char = '\u{2066}';
const RIGHT_TO_LEFT_ISOLATE: char = '\u{2067}';
const FIRST_STRONG_ISOLATE: char = '\u{2068}';
const POP_DIRECTIONAL_ISOLATE: char = '\u{2069}';

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidiRenderContract {
    pub direction: Direction,
    pub dir_attribute: &'static str,
    pub isolated_text: String,
}

impl BidiRenderContract {
    pub fn new(value: &str, direction: Direction) -> Self {
        Self {
            direction,
            dir_attribute: direction_attribute(direction),
            isolated_text: isolate_for_display(value, direction),
        }
    }
}

pub const fn direction_attribute(direction: Direction) -> &'static str {
    match direction {
        Direction::Auto => "auto",
        Direction::LeftToRight => "ltr",
        Direction::RightToLeft => "rtl",
    }
}

pub fn isolate_for_display(value: &str, direction: Direction) -> String {
    let isolate = match direction {
        Direction::Auto => FIRST_STRONG_ISOLATE,
        Direction::LeftToRight => LEFT_TO_RIGHT_ISOLATE,
        Direction::RightToLeft => RIGHT_TO_LEFT_ISOLATE,
    };
    let mut isolated = String::with_capacity(value.len() + 6);
    isolated.push(isolate);
    isolated.push_str(value);
    isolated.push(POP_DIRECTIONAL_ISOLATE);
    isolated
}

#[cfg(test)]
mod tests {
    use meiki_domain::Direction;

    use super::{BidiRenderContract, direction_attribute, isolate_for_display};

    #[test]
    fn explicit_directions_map_to_safe_rendering_attributes() {
        assert_eq!(direction_attribute(Direction::LeftToRight), "ltr");
        assert_eq!(direction_attribute(Direction::RightToLeft), "rtl");
        assert_eq!(direction_attribute(Direction::Auto), "auto");
    }

    #[test]
    fn content_is_isolated_without_changing_the_original_value() {
        let source = "Meeting الساعة 3:00!";
        let contract = BidiRenderContract::new(source, Direction::Auto);
        assert_eq!(source, "Meeting الساعة 3:00!");
        assert_eq!(
            contract.isolated_text,
            "\u{2068}Meeting الساعة 3:00!\u{2069}"
        );
        assert_eq!(
            isolate_for_display("کتاب", Direction::RightToLeft),
            "\u{2067}کتاب\u{2069}"
        );
    }
}
