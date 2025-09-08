use cogex_core::Stimulus;
pub struct Trial<S: Stimulus, T> {
    pub id: usize,
    pub stimulus: S,
    pub position: (f32, f32),
    pub durations: TrialDurations,
    pub timestamps: TrialTimestamps<T>,
    pub state: cogex_core::TrialState,
}

#[derive(Debug, Clone)]
pub struct TrialDurations {
    pub fixation_ms: u64,
    pub stimulus_ms: u64,
    pub response_window_ms: u64,
    pub feedback_ms: u64,
}

#[derive(Debug, Clone)]
pub struct TrialTimestamps<T> {
    pub start: T,
    pub fixation_start: T,
    pub stimulus_start: Option<T>,
    pub response: Option<T>,
}
