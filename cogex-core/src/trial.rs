use serde::{Deserialize, Serialize};

/// Trial state machine events
#[derive(Debug, Clone, PartialEq)]
pub enum TrialState {
    Fixation,
    Stimulus,
    Response,
    Feedback,
    Complete,
}

/// Recorded result per trial
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult<S> {
    pub trial_id: usize,
    pub stimulus_type: String,
    pub reaction_time_ns: Option<u64>,
    pub correct: Option<bool>,
    pub timestamp_ns: u64,
    #[serde(skip)]
    pub _marker: std::marker::PhantomData<S>,
}
