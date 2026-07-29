#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CompositionPhase {
    #[default]
    Idle,
    Composing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompositionEvent<'a> {
    Start,
    Update(&'a str),
    End(&'a str),
    Cancel,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompositionState {
    phase: CompositionPhase,
    draft: String,
}

impl CompositionState {
    pub const fn phase(&self) -> CompositionPhase {
        self.phase
    }

    pub fn draft(&self) -> &str {
        &self.draft
    }

    pub const fn is_composing(&self) -> bool {
        matches!(self.phase, CompositionPhase::Composing)
    }

    pub const fn allows_submission(&self, event_reports_composing: bool) -> bool {
        !self.is_composing() && !event_reports_composing
    }

    pub fn apply(&mut self, event: CompositionEvent<'_>) -> Option<String> {
        match event {
            CompositionEvent::Start => {
                self.phase = CompositionPhase::Composing;
                self.draft.clear();
                None
            }
            CompositionEvent::Update(value) => {
                if self.is_composing() {
                    value.clone_into(&mut self.draft);
                }
                None
            }
            CompositionEvent::End(value) => {
                self.phase = CompositionPhase::Idle;
                self.draft.clear();
                Some(value.to_owned())
            }
            CompositionEvent::Cancel => {
                self.phase = CompositionPhase::Idle;
                self.draft.clear();
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CompositionEvent, CompositionPhase, CompositionState};

    #[test]
    fn composition_text_is_opaque_until_commit() {
        let mut state = CompositionState::default();
        assert_eq!(state.apply(CompositionEvent::Start), None);
        assert!(!state.allows_submission(false));
        assert_eq!(state.apply(CompositionEvent::Update("にち")), None);
        assert_eq!(state.draft(), "にち");
        assert_eq!(
            state.apply(CompositionEvent::End("日")),
            Some("日".to_owned())
        );
        assert_eq!(state.phase(), CompositionPhase::Idle);
        assert!(state.allows_submission(false));
        assert!(!state.allows_submission(true));
    }
}
