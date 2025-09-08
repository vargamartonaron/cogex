use std::marker::PhantomData;

use cogex_core::Phase;

#[derive(Debug, Clone)]
pub struct ExperimentConfig<P: Phase> {
    pub practice_trials: usize,
    pub experiment_trials: usize,
    pub fixation_range_ms: (u64, u64),
    pub stimulus_duration_ms: u64,
    pub response_window_ms: u64,
    pub feedback_duration_ms: u64,
    pub inter_trial_interval_ms: u64,
    _phantom: std::marker::PhantomData<P>,
}

impl<P: Phase> Default for ExperimentConfig<P> {
    fn default() -> Self {
        /* same as before */
        Self {
            practice_trials: 20,
            experiment_trials: 100,
            fixation_range_ms: (500, 1500),
            stimulus_duration_ms: 200,
            response_window_ms: 2000,
            feedback_duration_ms: 500,
            inter_trial_interval_ms: 1000,
            _phantom: PhantomData,
        }
    }
}
