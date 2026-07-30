use std::fmt::Write;

use meiki_domain::{CardLifecycle, Grade, ScheduleState};

use crate::{ENGINE_VERSION, ScheduleDecision, SchedulerEngine, SchedulerError};

const MILLISECONDS_PER_DAY: f64 = 86_400_000.0;
const MINIMUM_INTERVAL_MILLISECONDS: u64 = 60_000;
const MINIMUM_STABILITY_DAYS: f64 = 0.0001;
const MAXIMUM_STABILITY_DAYS: f64 = 36_500.0;

pub const PARAMETER_COUNT: usize = 35;

/// Population defaults pinned from the FSRS-7 reference model.
pub const DEFAULT_PARAMETERS: [f64; PARAMETER_COUNT] = [
    0.041, 2.4175, 4.1283, 11.9709, 5.6385, 0.4468, 3.262, 2.3054, 0.1688, 1.3325, 0.3524, 0.0049,
    0.7503, 0.0896, 0.6625, 1.3, 0.882, 0.3072, 3.5875, 0.303, 0.0107, 0.2279, 2.6413, 0.5594, 1.3,
    2.5, 1.0, 0.0723, 0.1634, 0.5, 0.9555, 0.2245, 0.6232, 0.1362, 0.3862,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchedulerConfig {
    pub target_retention_basis_points: u16,
    pub maximum_interval_days: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            target_retention_basis_points: 9_000,
            maximum_interval_days: 36_500,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fsrs7Engine {
    parameters: [f64; PARAMETER_COUNT],
    config: SchedulerConfig,
}

#[derive(Clone, Copy, Debug)]
struct MemoryState {
    stability_days: f64,
    difficulty: f64,
    last_reviewed_at_ms: i64,
}

impl Fsrs7Engine {
    /// Creates the bundled balanced engine.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied configuration is outside supported
    /// safety bounds.
    pub fn new(config: SchedulerConfig) -> Result<Self, SchedulerError> {
        Self::from_parameters(config, &DEFAULT_PARAMETERS)
    }

    /// Creates an engine from an explicitly versioned parameter vector.
    ///
    /// # Errors
    ///
    /// Returns an error for the wrong parameter count, non-finite weights, or
    /// unsafe configuration values.
    pub fn from_parameters(
        config: SchedulerConfig,
        parameters: &[f64],
    ) -> Result<Self, SchedulerError> {
        validate_config(config)?;
        let parameters: [f64; PARAMETER_COUNT] =
            parameters
                .try_into()
                .map_err(|_| SchedulerError::InvalidParameterCount {
                    expected: PARAMETER_COUNT,
                    found: parameters.len(),
                })?;
        for (index, value) in parameters.iter().enumerate() {
            if !value.is_finite() {
                return Err(SchedulerError::InvalidParameter(index));
            }
        }
        validate_parameters(&parameters)?;
        Ok(Self { parameters, config })
    }

    pub const fn parameters(&self) -> &[f64; PARAMETER_COUNT] {
        &self.parameters
    }

    pub const fn config(&self) -> SchedulerConfig {
        self.config
    }

    /// Restores a bit-exact parameter vector produced by
    /// [`SchedulerEngine::serialize_parameters`].
    ///
    /// # Errors
    ///
    /// Returns an error when the version, count, or hexadecimal values are
    /// invalid.
    pub fn deserialize_parameters(
        config: SchedulerConfig,
        serialized: &str,
    ) -> Result<Self, SchedulerError> {
        let encoded =
            serialized
                .strip_prefix("fsrs-7:")
                .ok_or(SchedulerError::InvalidSerialization(
                    "parameter version is not fsrs-7",
                ))?;
        let values = encoded
            .split(',')
            .map(|value| {
                u64::from_str_radix(value, 16)
                    .map(f64::from_bits)
                    .map_err(|_| {
                        SchedulerError::InvalidSerialization(
                            "parameter value is not hexadecimal f64 bits",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::from_parameters(config, &values)
    }

    fn target_retention(&self) -> f64 {
        f64::from(self.config.target_retention_basis_points) / 10_000.0
    }

    fn memory_from_schedule(state: &ScheduleState) -> Result<Option<MemoryState>, SchedulerError> {
        match (
            state.stability_milliseconds,
            state.difficulty_millipoints,
            state.last_reviewed_at_ms,
        ) {
            (0, 0, None) => Ok(None),
            (stability, difficulty, Some(last_reviewed_at_ms))
                if stability > 0 && (1_000..=10_000).contains(&difficulty) =>
            {
                Ok(Some(MemoryState {
                    stability_days: milliseconds_to_days(stability),
                    difficulty: f64::from(difficulty) / 1_000.0,
                    last_reviewed_at_ms,
                }))
            }
            _ => Err(SchedulerError::InvalidState(
                "memory fields must be entirely empty or initialized",
            )),
        }
    }

    fn updated_memory(
        &self,
        current: Option<MemoryState>,
        grade: Grade,
        reviewed_at_ms: i64,
    ) -> Result<MemoryState, SchedulerError> {
        let rating = rating(grade);
        let Some(current) = current else {
            return Ok(MemoryState {
                stability_days: self.parameters[usize::from(rating - 1)],
                difficulty: self.initial_difficulty(rating).clamp(1.0, 10.0),
                last_reviewed_at_ms: reviewed_at_ms,
            });
        };
        if reviewed_at_ms < current.last_reviewed_at_ms {
            return Err(SchedulerError::InvalidState(
                "review timestamp precedes the previous review",
            ));
        }
        let elapsed_days = elapsed_days(reviewed_at_ms, current.last_reviewed_at_ms)?;
        let retrievability = self.forgetting_curve(elapsed_days, current.stability_days);
        let long_term = self.stability_after_review(current, retrievability, rating, 7);
        let short_term = self.stability_after_review(current, retrievability, rating, 16);
        let transition = (1.0 - self.parameters[26] * (-self.parameters[25] * elapsed_days).exp())
            .clamp(0.0, 1.0);
        let stability_days = (transition * long_term + (1.0 - transition) * short_term)
            .clamp(MINIMUM_STABILITY_DAYS, MAXIMUM_STABILITY_DAYS);
        Ok(MemoryState {
            stability_days,
            difficulty: self
                .next_difficulty(current.difficulty, rating)
                .clamp(1.0, 10.0),
            last_reviewed_at_ms: reviewed_at_ms,
        })
    }

    fn stability_after_review(
        &self,
        current: MemoryState,
        retrievability: f64,
        rating: u8,
        base: usize,
    ) -> f64 {
        let parameters = &self.parameters;
        let failed = parameters[base + 3]
            * current.difficulty.powf(-parameters[base + 4])
            * ((current.stability_days + 1.0).powf(parameters[base + 5]) - 1.0)
            * ((1.0 - retrievability) * parameters[base + 6]).exp();
        let post_lapse = current.stability_days.min(failed);
        if rating == 1 {
            return post_lapse;
        }
        let hard_penalty = if rating == 2 {
            parameters[base + 7]
        } else {
            1.0
        };
        let easy_bonus = if rating == 4 {
            parameters[base + 8]
        } else {
            1.0
        };
        let increase = 1.0
            + (parameters[base] - 1.5).exp()
                * (11.0 - current.difficulty)
                * current.stability_days.powf(-parameters[base + 1])
                * (((1.0 - retrievability) * parameters[base + 2]).exp() - 1.0)
                * hard_penalty
                * easy_bonus;
        post_lapse.max(current.stability_days * increase)
    }

    fn initial_difficulty(&self, rating: u8) -> f64 {
        self.parameters[4] - (self.parameters[5] * (f64::from(rating) - 1.0)).exp() + 1.0
    }

    fn next_difficulty(&self, current: f64, rating: u8) -> f64 {
        let delta = -self.parameters[6] * (f64::from(rating) - 3.0);
        let damped = delta * (10.0 - current) / 9.0;
        0.01 * self.initial_difficulty(4) + 0.99 * (current + damped)
    }

    fn forgetting_curve(&self, elapsed_days: f64, stability_days: f64) -> f64 {
        forgetting_curve(&self.parameters, elapsed_days, stability_days)
    }

    fn interval_milliseconds(&self, stability_days: f64) -> u64 {
        let maximum_days = f64::from(self.config.maximum_interval_days);
        let target = self.target_retention();
        let mut low = 0.0_f64;
        let mut high = maximum_days;
        if self.forgetting_curve(high, stability_days) > target {
            return days_to_milliseconds(high);
        }
        for _ in 0..80 {
            let midpoint = low.midpoint(high);
            if self.forgetting_curve(midpoint, stability_days) > target {
                low = midpoint;
            } else {
                high = midpoint;
            }
        }
        days_to_milliseconds(low.midpoint(high)).max(MINIMUM_INTERVAL_MILLISECONDS)
    }

    fn schedule_from_memory(
        &self,
        current: &ScheduleState,
        memory: MemoryState,
        grade: Grade,
    ) -> ScheduleState {
        let interval_milliseconds = self.interval_milliseconds(memory.stability_days);
        let ideal_due_at_ms = memory
            .last_reviewed_at_ms
            .saturating_add(i64::try_from(interval_milliseconds).unwrap_or(i64::MAX));
        ScheduleState {
            card_id: current.card_id.clone(),
            version: current.version.saturating_add(1),
            lifecycle: CardLifecycle::Introduced,
            due_at_ms: ideal_due_at_ms,
            ideal_due_at_ms,
            interval_milliseconds,
            interval_seconds: interval_milliseconds.saturating_add(999) / 1_000,
            repetitions: if grade == Grade::Again {
                0
            } else {
                current.repetitions.saturating_add(1)
            },
            stability_milliseconds: days_to_milliseconds(memory.stability_days),
            difficulty_millipoints: difficulty_to_millipoints(memory.difficulty),
            last_reviewed_at_ms: Some(memory.last_reviewed_at_ms),
            last_review_event_id: None,
        }
    }
}

impl SchedulerEngine for Fsrs7Engine {
    fn version(&self) -> &'static str {
        ENGINE_VERSION
    }

    fn initial_schedule(&self, card_id: &str, created_at_ms: i64) -> ScheduleState {
        ScheduleState {
            card_id: card_id.to_owned(),
            version: 0,
            lifecycle: CardLifecycle::Unseen,
            due_at_ms: created_at_ms,
            ideal_due_at_ms: created_at_ms,
            interval_milliseconds: 0,
            interval_seconds: 0,
            repetitions: 0,
            stability_milliseconds: 0,
            difficulty_millipoints: 0,
            last_reviewed_at_ms: None,
            last_review_event_id: None,
        }
    }

    fn review(
        &self,
        current: &ScheduleState,
        grade: Grade,
        reviewed_at_ms: i64,
    ) -> Result<ScheduleDecision, SchedulerError> {
        let memory =
            self.updated_memory(Self::memory_from_schedule(current)?, grade, reviewed_at_ms)?;
        Ok(ScheduleDecision {
            scheduler_version: ENGINE_VERSION,
            target_retention_basis_points: self.config.target_retention_basis_points,
            next_state: self.schedule_from_memory(current, memory, grade),
        })
    }

    fn recall_probability(&self, state: &ScheduleState, at_ms: i64) -> Result<f64, SchedulerError> {
        let memory = Self::memory_from_schedule(state)?.ok_or(SchedulerError::InvalidState(
            "recall probability is undefined before the first review",
        ))?;
        if at_ms < memory.last_reviewed_at_ms {
            return Err(SchedulerError::InvalidState(
                "query timestamp precedes the previous review",
            ));
        }
        let elapsed_days = elapsed_days(at_ms, memory.last_reviewed_at_ms)?;
        Ok(self.forgetting_curve(elapsed_days, memory.stability_days))
    }

    fn serialize_parameters(&self) -> String {
        let mut serialized = String::from("fsrs-7:");
        for (index, parameter) in self.parameters.iter().enumerate() {
            if index > 0 {
                serialized.push(',');
            }
            write!(&mut serialized, "{:016x}", parameter.to_bits())
                .expect("writing to a String cannot fail");
        }
        serialized
    }

    fn serialize_state(&self, state: &ScheduleState) -> String {
        format!(
            "fsrs-7|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            encode_text(&state.card_id),
            state.version,
            state.due_at_ms,
            state.ideal_due_at_ms,
            state.interval_milliseconds,
            state.interval_seconds,
            state.repetitions,
            state.stability_milliseconds,
            state.difficulty_millipoints,
            state
                .last_reviewed_at_ms
                .map_or_else(|| "-".to_owned(), |value| value.to_string()),
            state
                .last_review_event_id
                .as_deref()
                .map_or_else(|| "-".to_owned(), encode_text),
            match state.lifecycle {
                CardLifecycle::Unseen => "unseen",
                CardLifecycle::Introduced => "introduced",
            }
        )
    }

    fn deserialize_state(&self, serialized: &str) -> Result<ScheduleState, SchedulerError> {
        let fields = serialized.split('|').collect::<Vec<_>>();
        if !matches!(fields.len(), 12 | 13) || fields[0] != ENGINE_VERSION {
            return Err(SchedulerError::InvalidSerialization(
                "state version or field count is invalid",
            ));
        }
        let lifecycle = if let Some(lifecycle) = fields.get(12) {
            match *lifecycle {
                "unseen" => CardLifecycle::Unseen,
                "introduced" => CardLifecycle::Introduced,
                _ => {
                    return Err(SchedulerError::InvalidSerialization(
                        "card lifecycle is invalid",
                    ));
                }
            }
        } else if parse_field::<u32>(fields[7])? > 0
            || parse_field::<u64>(fields[8])? > 0
            || parse_field::<u32>(fields[9])? > 0
            || fields[10] != "-"
        {
            CardLifecycle::Introduced
        } else {
            CardLifecycle::Unseen
        };
        let state = ScheduleState {
            card_id: decode_text(fields[1])?,
            version: parse_field(fields[2])?,
            lifecycle,
            due_at_ms: parse_field(fields[3])?,
            ideal_due_at_ms: parse_field(fields[4])?,
            interval_milliseconds: parse_field(fields[5])?,
            interval_seconds: parse_field(fields[6])?,
            repetitions: parse_field(fields[7])?,
            stability_milliseconds: parse_field(fields[8])?,
            difficulty_millipoints: parse_field(fields[9])?,
            last_reviewed_at_ms: parse_optional_field(fields[10])?,
            last_review_event_id: if fields[11] == "-" {
                None
            } else {
                Some(decode_text(fields[11])?)
            },
        };
        Self::memory_from_schedule(&state)?;
        if state.interval_seconds != state.interval_milliseconds.saturating_add(999) / 1_000 {
            return Err(SchedulerError::InvalidSerialization(
                "state interval fields are inconsistent",
            ));
        }
        Ok(state)
    }
}

pub(crate) fn forgetting_curve(
    parameters: &[f64; PARAMETER_COUNT],
    elapsed_days: f64,
    stability_days: f64,
) -> f64 {
    let stability_days = stability_days.max(MINIMUM_STABILITY_DAYS);
    let ratio = elapsed_days.max(0.0) / stability_days;
    let first = power_law(parameters[29], -parameters[27], ratio);
    let second = power_law(parameters[30], -parameters[28], ratio);
    let first_weight = parameters[31] * stability_days.powf(-parameters[33]);
    let second_weight = parameters[32] * stability_days.powf(parameters[34]);
    ((first_weight * first + second_weight * second) / (first_weight + second_weight))
        .clamp(0.0, 1.0)
}

fn power_law(base: f64, decay: f64, ratio: f64) -> f64 {
    let factor = base.powf(1.0 / decay) - 1.0;
    (1.0 + factor * ratio).powf(decay)
}

fn encode_text(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn decode_text(value: &str) -> Result<String, SchedulerError> {
    let pairs = value.as_bytes().chunks_exact(2);
    if !pairs.remainder().is_empty() {
        return Err(SchedulerError::InvalidSerialization(
            "state text has invalid hexadecimal length",
        ));
    }
    let bytes = pairs
        .map(|pair| {
            let high = decode_nibble(pair[0])?;
            let low = decode_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect::<Result<Vec<_>, SchedulerError>>()?;
    String::from_utf8(bytes)
        .map_err(|_| SchedulerError::InvalidSerialization("state text is not valid UTF-8"))
}

const fn decode_nibble(value: u8) -> Result<u8, SchedulerError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(SchedulerError::InvalidSerialization(
            "state text is not hexadecimal",
        )),
    }
}

fn parse_field<T>(value: &str) -> Result<T, SchedulerError>
where
    T: std::str::FromStr,
{
    value
        .parse()
        .map_err(|_| SchedulerError::InvalidSerialization("state number is invalid"))
}

fn parse_optional_field<T>(value: &str) -> Result<Option<T>, SchedulerError>
where
    T: std::str::FromStr,
{
    if value == "-" {
        Ok(None)
    } else {
        parse_field(value).map(Some)
    }
}

fn rating(grade: Grade) -> u8 {
    match grade {
        Grade::Again => 1,
        Grade::Hard => 2,
        Grade::Good => 3,
        Grade::Easy => 4,
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn days_to_milliseconds(days: f64) -> u64 {
    (days.clamp(0.0, MAXIMUM_STABILITY_DAYS) * MILLISECONDS_PER_DAY).round() as u64
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn difficulty_to_millipoints(difficulty: f64) -> u32 {
    (difficulty.clamp(1.0, 10.0) * 1_000.0).round() as u32
}

fn milliseconds_to_days(milliseconds: u64) -> f64 {
    std::time::Duration::from_millis(milliseconds).as_secs_f64() / 86_400.0
}

fn elapsed_days(later_ms: i64, earlier_ms: i64) -> Result<f64, SchedulerError> {
    let milliseconds = later_ms
        .checked_sub(earlier_ms)
        .filter(|value| *value >= 0)
        .ok_or(SchedulerError::InvalidState(
            "timestamp difference is outside supported bounds",
        ))?;
    let milliseconds = u64::try_from(milliseconds).map_err(|_| {
        SchedulerError::InvalidState("timestamp difference is outside supported bounds")
    })?;
    Ok(milliseconds_to_days(milliseconds))
}

fn validate_config(config: SchedulerConfig) -> Result<(), SchedulerError> {
    if !(7_000..=9_900).contains(&config.target_retention_basis_points) {
        return Err(SchedulerError::InvalidTargetRetention(
            config.target_retention_basis_points,
        ));
    }
    if !(1..=36_500).contains(&config.maximum_interval_days) {
        return Err(SchedulerError::InvalidMaximumInterval(
            config.maximum_interval_days,
        ));
    }
    Ok(())
}

fn validate_parameters(parameters: &[f64; PARAMETER_COUNT]) -> Result<(), SchedulerError> {
    const BOUNDS: [(f64, f64); PARAMETER_COUNT] = [
        (0.0001, 50.0),
        (0.0001, 100.0),
        (0.0001, 100.0),
        (0.0001, 100.0),
        (1.0, 10.0),
        (0.001, 4.0),
        (0.1, 4.0),
        (0.0, 4.0),
        (0.0, 1.2),
        (0.3, 3.0),
        (0.01, 1.5),
        (0.001, 0.9),
        (0.1, 1.0),
        (0.0, 3.5),
        (0.0, 1.0),
        (1.0, 7.0),
        (0.0, 4.0),
        (0.0, 2.0),
        (0.5, 6.0),
        (0.001, 1.5),
        (0.001, 2.0),
        (0.001, 1.0),
        (0.0, 5.0),
        (0.0, 1.0),
        (1.0, 7.0),
        (2.5, 15.0),
        (0.0, 1.0),
        (0.01, 0.25),
        (0.01, 0.95),
        (0.5, 0.85),
        (0.5, 0.99),
        (0.01, 1.0),
        (0.1, 1.0),
        (0.0, 0.9),
        (0.1, 1.1),
    ];
    for (index, (value, (minimum, maximum))) in parameters.iter().zip(BOUNDS).enumerate() {
        if !(minimum..=maximum).contains(value) {
            return Err(SchedulerError::InvalidParameter(index));
        }
    }
    if parameters[0] > parameters[1]
        || parameters[1] > parameters[2]
        || parameters[2] > parameters[3]
        || parameters[27] > parameters[28]
        || parameters[29] > parameters[30]
    {
        return Err(SchedulerError::InvalidParameterOrder);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use meiki_domain::{CardLifecycle, Grade};

    use crate::{SchedulerEngine, SchedulerError};

    use super::{Fsrs7Engine, SchedulerConfig};

    const DAY_MS: i64 = 86_400_000;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "expected {expected:.15}, found {actual:.15}"
        );
    }

    #[test]
    fn pinned_reference_vector_matches_fsrs7_model() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let mut memory = engine.updated_memory(None, Grade::Good, 0).unwrap();
        assert_close(memory.stability_days, 4.1283);
        assert_close(memory.difficulty, 4.194_588_083_372_719);

        memory = engine
            .updated_memory(Some(memory), Grade::Good, DAY_MS / 4)
            .unwrap();
        assert_close(memory.stability_days, 7.273_941_032_621_495_5);
        assert_close(memory.difficulty, 4.180_821_488_255_665);

        memory = engine
            .updated_memory(Some(memory), Grade::Again, DAY_MS / 4 + 7 * DAY_MS)
            .unwrap();
        assert_close(memory.stability_days, 1.369_882_024_580_990_2);
        assert_close(memory.difficulty, 8.343_267_826_257_986);

        memory = engine
            .updated_memory(Some(memory), Grade::Hard, DAY_MS / 4 + 7 * DAY_MS)
            .unwrap();
        assert_close(memory.stability_days, 1.369_882_024_580_990_2);
        assert_close(memory.difficulty, 8.882_483_072_294_189);

        memory = engine
            .updated_memory(Some(memory), Grade::Easy, DAY_MS / 4 + 127 * DAY_MS)
            .unwrap();
        assert_close(memory.stability_days, 8.413_020_800_712_248);
        assert_close(memory.difficulty, 8.420_850_103_288_52);
    }

    #[test]
    fn reference_interval_is_fractional_and_timestamp_based() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let interval = engine.interval_milliseconds(4.1283);
        assert_eq!(interval, 256_342_507);
        assert_ne!(interval % (DAY_MS as u64), 0);

        let scheduled = engine
            .review(&engine.initial_schedule("card-1", 500), Grade::Good, 1_000)
            .unwrap()
            .next_state;
        assert_eq!(scheduled.interval_milliseconds, interval);
        assert_eq!(scheduled.ideal_due_at_ms, 256_343_507);
        assert_eq!(scheduled.due_at_ms, scheduled.ideal_due_at_ms);
    }

    #[test]
    fn simulations_remain_finite_for_same_day_lapse_overdue_and_long_history() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let mut state = engine.initial_schedule("card-1", 0);
        let mut reviewed_at_ms = 0;
        for review in 0..2_000 {
            let elapsed = match review % 5 {
                0 => 0,
                1 => 10 * 60 * 1_000,
                2 => DAY_MS,
                3 => 365 * DAY_MS,
                _ => 7 * DAY_MS,
            };
            reviewed_at_ms += elapsed;
            let grade = match review % 11 {
                0 => Grade::Again,
                1 => Grade::Hard,
                10 => Grade::Easy,
                _ => Grade::Good,
            };
            state = engine
                .review(&state, grade, reviewed_at_ms)
                .unwrap()
                .next_state;
            let recall = engine
                .recall_probability(&state, reviewed_at_ms.saturating_add(30 * DAY_MS))
                .unwrap();
            assert!(recall.is_finite() && (0.0..=1.0).contains(&recall));
            assert!(state.interval_milliseconds > 0);
            assert!(state.due_at_ms >= reviewed_at_ms);
            assert!(state.ideal_due_at_ms >= reviewed_at_ms);
            assert!(state.stability_milliseconds > 0);
            assert!((1_000..=10_000).contains(&state.difficulty_millipoints));
        }
    }

    #[test]
    fn memory_transition_rejects_incomplete_state() {
        assert!(matches!(
            Fsrs7Engine::memory_from_schedule(&meiki_domain::ScheduleState {
                card_id: "card-1".to_owned(),
                version: 0,
                lifecycle: CardLifecycle::Unseen,
                due_at_ms: 0,
                ideal_due_at_ms: 0,
                interval_milliseconds: 0,
                interval_seconds: 0,
                repetitions: 0,
                stability_milliseconds: 1,
                difficulty_millipoints: 0,
                last_reviewed_at_ms: None,
                last_review_event_id: None,
            }),
            Err(SchedulerError::InvalidState(_))
        ));
    }

    #[test]
    fn parameter_and_state_serialization_is_deterministic() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let serialized = engine.serialize_parameters();
        let restored =
            Fsrs7Engine::deserialize_parameters(SchedulerConfig::default(), &serialized).unwrap();
        assert!(
            restored
                .parameters()
                .iter()
                .zip(engine.parameters())
                .all(|(left, right)| left.to_bits() == right.to_bits())
        );

        let initial = engine.initial_schedule("card-1", 1_000);
        assert_eq!(
            engine.serialize_state(&initial),
            engine.serialize_state(&initial)
        );
        let unicode = engine.initial_schedule("card|日本語", 1_000);
        assert_eq!(
            engine
                .deserialize_state(&engine.serialize_state(&unicode))
                .unwrap(),
            unicode
        );
        let current = engine.serialize_state(&unicode);
        let legacy = current
            .rsplit_once('|')
            .expect("serialized lifecycle suffix")
            .0;
        assert_eq!(engine.deserialize_state(legacy).unwrap(), unicode);
        assert_eq!(
            engine.review(&initial, Grade::Good, 2_000).unwrap(),
            engine.review(&initial, Grade::Good, 2_000).unwrap()
        );
    }

    #[test]
    fn first_review_and_lapse_keep_lifecycle_independent_from_repetitions() {
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let initial = engine.initial_schedule("card-1", 1_000);
        assert_eq!(initial.lifecycle, CardLifecycle::Unseen);

        let first_again = engine
            .review(&initial, Grade::Again, 2_000)
            .unwrap()
            .next_state;
        assert_eq!(first_again.repetitions, 0);
        assert_eq!(first_again.lifecycle, CardLifecycle::Introduced);

        let mature = engine
            .review(&initial, Grade::Good, 2_000)
            .unwrap()
            .next_state;
        let lapsed = engine
            .review(&mature, Grade::Again, mature.due_at_ms)
            .unwrap()
            .next_state;
        assert_eq!(lapsed.repetitions, 0);
        assert_eq!(lapsed.lifecycle, CardLifecycle::Introduced);
    }

    #[test]
    fn invalid_configuration_and_time_are_rejected() {
        assert!(matches!(
            Fsrs7Engine::new(SchedulerConfig {
                target_retention_basis_points: 10_000,
                maximum_interval_days: 365,
            }),
            Err(SchedulerError::InvalidTargetRetention(10_000))
        ));
        let engine = Fsrs7Engine::new(SchedulerConfig::default()).unwrap();
        let first = engine
            .review(
                &engine.initial_schedule("card-1", 1_000),
                Grade::Good,
                10_000,
            )
            .unwrap();
        assert!(
            engine
                .review(&first.next_state, Grade::Good, 9_999)
                .is_err()
        );
    }
}
