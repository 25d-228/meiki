//! Pure, versioned scheduling engines and time-budget policy control.
//!
//! This crate owns no UI, SQL, clock, filesystem, or mutable global state.

mod controller;
mod fsrs7;

use std::fmt;

use meiki_domain::{Grade, ScheduleState};

pub use controller::{
    AutomaticPolicyDecision, AutomaticPolicyInput, BASELINE_TARGET_RETENTION_BASIS_POINTS,
    CONTROLLER_VERSION, DeckIntakeAllocation, DeckIntakeCandidate, FORECAST_DAYS,
    MAXIMUM_TARGET_RETENTION_BASIS_POINTS, MINIMUM_TARGET_RETENTION_BASIS_POINTS,
    allocate_unseen_round_robin, automatic_policy,
};
pub use fsrs7::{DEFAULT_PARAMETERS, Fsrs7Engine, PARAMETER_COUNT, SchedulerConfig};

pub const ENGINE_VERSION: &str = "fsrs-7";
pub const DEFAULT_PARAMETER_SET_ID: &str = "fsrs7-default-v1";

#[derive(Clone, Debug, PartialEq)]
pub struct ScheduleDecision {
    pub scheduler_version: &'static str,
    pub target_retention_basis_points: u16,
    pub next_state: ScheduleState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    InvalidParameterCount { expected: usize, found: usize },
    InvalidParameter(usize),
    InvalidParameterOrder,
    InvalidTargetRetention(u16),
    InvalidMaximumInterval(u32),
    InvalidState(&'static str),
    InvalidSerialization(&'static str),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidParameterCount { expected, found } => {
                write!(
                    formatter,
                    "expected {expected} scheduler parameters, found {found}"
                )
            }
            Self::InvalidParameter(index) => {
                write!(
                    formatter,
                    "scheduler parameter {index} is outside safe bounds"
                )
            }
            Self::InvalidParameterOrder => {
                formatter.write_str("scheduler parameters violate required ordering")
            }
            Self::InvalidTargetRetention(value) => {
                write!(
                    formatter,
                    "target retention {value} must be between 7000 and 9900 basis points"
                )
            }
            Self::InvalidMaximumInterval(value) => {
                write!(
                    formatter,
                    "maximum interval {value} must be between 1 and 36500 days"
                )
            }
            Self::InvalidState(reason) => write!(formatter, "invalid schedule state: {reason}"),
            Self::InvalidSerialization(reason) => {
                write!(formatter, "invalid scheduler serialization: {reason}")
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Replaceable pure scheduling boundary.
pub trait SchedulerEngine {
    fn version(&self) -> &'static str;

    /// Returns an immediately due, version-zero state for a new card.
    fn initial_schedule(&self, card_id: &str, created_at_ms: i64) -> ScheduleState;

    /// Applies one graded review without performing I/O.
    ///
    /// # Errors
    ///
    /// Returns an error when the persisted state is inconsistent or time moves
    /// backwards.
    fn review(
        &self,
        current: &ScheduleState,
        grade: Grade,
        reviewed_at_ms: i64,
    ) -> Result<ScheduleDecision, SchedulerError>;

    /// Predicts recall probability at an exact timestamp.
    ///
    /// # Errors
    ///
    /// Returns an error when the card has no initialized memory state or time
    /// moves backwards.
    fn recall_probability(&self, state: &ScheduleState, at_ms: i64) -> Result<f64, SchedulerError>;

    /// Serializes parameters in a canonical, bit-exact representation.
    fn serialize_parameters(&self) -> String;

    /// Serializes a projected state without locale-sensitive formatting.
    fn serialize_state(&self, state: &ScheduleState) -> String;

    /// Restores a projected state from the engine's canonical serialization.
    ///
    /// # Errors
    ///
    /// Returns an error when the version, field count, encoding, or memory
    /// invariants are invalid.
    fn deserialize_state(&self, serialized: &str) -> Result<ScheduleState, SchedulerError>;
}
