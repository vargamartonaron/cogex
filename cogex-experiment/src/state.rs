use super::config::ExperimentConfig;
use super::trial::{Trial, TrialDurations, TrialTimestamps};
use cogex_core::{ArrowDirection, Phase, Stimulus, StimulusType, TrialResult, TrialState};
use cogex_timing::Timer;
use rand::Rng;
use std::marker::PhantomData;
use std::time::Duration;

pub struct ExperimentStateMachine<P, S, T, R>
where
    P: Phase,
    S: Stimulus,
    T: Timer,
    R: Rng,
{
    pub phase: P,
    pub timer: T,
    pub rng: R,
    pub config: ExperimentConfig<P>,
    pub current: Option<Trial<S, T::Timestamp>>,
    pub trial_number: usize,
    pub results: Vec<TrialResult<S>>,
    pub calibrated: bool,
    pub safe_margin_ns: u64,
}

impl<P, T, R> ExperimentStateMachine<P, StimulusType, T, R>
where
    P: Phase + Default,
    T: Timer<Timestamp = u64>,
    R: Rng,
{
    pub fn new(config: ExperimentConfig<P>, timer: T, rng: R) -> Self {
        Self {
            phase: P::default(), // Requires Phase: Default
            timer,
            rng,
            config,
            current: None,
            trial_number: 0,
            results: Vec::new(),
            calibrated: false,
            safe_margin_ns: 0,
        }
    }

    pub fn advance_phase(&mut self) -> bool {
        if let Some(next) = self.phase.next() {
            self.phase = next;
            true
        } else {
            false
        }
    }

    pub fn apply_calibration(&mut self) {
        let stats = self.timer.calibration_stats();
        self.safe_margin_ns = (stats.jitter_ns * 3.0) as u64;
        self.calibrated = true;
        // Add margin to stimulus duration for safety
        self.config.stimulus_duration_ms += self.safe_margin_ns / 1_000_000;
        println!(
            "Calibration: {:.3} ms/frame, {:.1} Hz, jitter {:.3} ms, safe margin {} ns",
            stats.average_frame_time_ns / 1_000_000.0,
            stats.effective_fps,
            stats.jitter_ns / 1_000_000.0,
            self.safe_margin_ns,
        );
    }

    pub fn start_trial(&mut self) {
        let id = self.trial_number;
        let stim = self.generate_stimulus();
        let pos = self.generate_position();
        let fixation_ms = self
            .rng
            .random_range(self.config.fixation_range_ms.0..=self.config.fixation_range_ms.1);
        let now_ns = self.timer.now() as u64;

        let trial = Trial {
            id,
            stimulus: stim,
            position: pos,
            durations: TrialDurations {
                fixation_ms,
                stimulus_ms: self.config.stimulus_duration_ms,
                response_window_ms: self.config.response_window_ms,
                feedback_ms: self.config.feedback_duration_ms,
            },
            timestamps: TrialTimestamps {
                start: now_ns,
                fixation_start: now_ns,
                stimulus_start: None,
                response: None,
            },
            state: TrialState::Fixation,
        };

        self.current = Some(trial);
        println!("Trial {} started at {:?} ns", id, now_ns);
    }

    pub fn update(&mut self) {
        if !self.calibrated {
            // Calibration not complete, skip trial updates
            return;
        }

        let now_ns = self.timer.now();

        if let Some(trial) = &mut self.current {
            match trial.state {
                TrialState::Fixation => {
                    if now_ns - trial.timestamps.fixation_start
                        >= trial.durations.fixation_ms * 1_000_000
                    {
                        trial.state = TrialState::Stimulus;
                        trial.timestamps.stimulus_start = Some(now_ns);
                        println!("Stimulus started at {}", now_ns);
                    }
                }
                TrialState::Stimulus => {
                    let dur_ns = trial.durations.stimulus_ms * 1_000_000 + self.safe_margin_ns;
                    if let Some(start_ns) = trial.timestamps.stimulus_start {
                        if now_ns - start_ns >= dur_ns {
                            trial.state = TrialState::Response;
                            println!("Response window opened at {}", now_ns);
                        }
                    }
                }
                TrialState::Response => {
                    let total_ns = (trial.durations.stimulus_ms
                        + trial.durations.response_window_ms)
                        * 1_000_000
                        + self.safe_margin_ns;
                    if let Some(start_ns) = trial.timestamps.stimulus_start {
                        if now_ns - start_ns >= total_ns {
                            self.complete_trial(None);
                        }
                    }
                }
                TrialState::Feedback => {
                    let total_ns = (trial.durations.fixation_ms
                        + trial.durations.stimulus_ms
                        + trial.durations.response_window_ms
                        + trial.durations.feedback_ms)
                        * 1_000_000
                        + self.safe_margin_ns;
                    if now_ns - trial.timestamps.start >= total_ns {
                        trial.state = TrialState::Complete;
                        self.next_trial();
                    }
                }
                TrialState::Complete => {
                    // waiting for next trial
                }
            }
        }
    }

    /// Records a response for the current trial during the Response state
    pub fn record_response(&mut self) {
        if let Some(trial) = &mut self.current {
            if trial.state == TrialState::Response {
                let now_ns = self.timer.now();
                trial.timestamps.response = Some(now_ns);
                trial.state = TrialState::Feedback;

                let rt = now_ns - trial.timestamps.stimulus_start.unwrap_or(now_ns);
                println!(
                    "Response recorded at {}, RT = {:.3} ms",
                    now_ns,
                    rt as f64 / 1_000_000.0
                );
                self.complete_trial(Some(now_ns));
            }
        }
    }

    /// Completes the current trial and stores the results
    fn complete_trial(&mut self, timestamp: Option<T::Timestamp>) {
        if let Some(trial) = &self.current {
            let reaction_ns = trial
                .timestamps
                .response
                .map(|r| r - trial.timestamps.stimulus_start.unwrap_or(r));
            let correct = reaction_ns.is_some();

            let result = TrialResult {
                trial_id: trial.id,
                stimulus_type: trial.stimulus.identifier().to_string(),
                reaction_time_ns: reaction_ns,
                correct: Some(correct),
                timestamp_ns: timestamp.unwrap_or_default(),
                _marker: PhantomData,
            };

            self.results.push(result);
        }
    }

    /// Proceeds to the next trial or advances phase upon completion
    fn next_trial(&mut self) {
        self.trial_number += 1;
        self.current = None;

        self.timer
            .sleep(Duration::from_millis(self.config.inter_trial_interval_ms));

        if self.phase.is_practice() /* Should match Practice phase */ && self.trial_number >= self.config.practice_trials
        {
            self.advance_phase();
        } else if self.phase.is_experiment() /* Should match Experiment phase */ && self.trial_number >= self.config.experiment_trials
        {
            self.advance_phase();
        } else {
            self.start_trial();
        }
    }

    fn generate_stimulus(&mut self) -> StimulusType {
        // Example: generate a random standard stimulus
        match self.rng.random_range(0..4) {
            0 => StimulusType::Circle {
                radius: 50.0,
                color: [255, 0, 0, 255],
            },
            1 => StimulusType::Rectangle {
                width: 80.0,
                height: 60.0,
                color: [0, 255, 0, 255],
            },
            2 => StimulusType::Arrow {
                direction: ArrowDirection::Right,
                size: 60.0,
                color: [0, 0, 255, 255],
            },
            _ => StimulusType::Text {
                content: "Test".to_string(),
                size: 24.0,
                color: [255, 255, 255, 255],
            },
        }
    }

    /// Generates stimulus position
    fn generate_position(&mut self) -> (f32, f32) {
        let x = self.rng.random_range(100.0..700.0);
        let y = self.rng.random_range(100.0..500.0);
        (x, y)
    }

    /// Returns true if experiment is calibrated
    pub fn is_calibrated(&self) -> bool {
        self.calibrated
    }

    /// Returns current phase
    pub fn current_phase(&self) -> &P {
        &self.phase
    }

    /// Returns current stimulus and position if any
    pub fn current_stimulus(&self) -> Option<(&StimulusType, (f32, f32))> {
        self.current.as_ref().map(|t| (&t.stimulus, t.position))
    }

    /// Experiment results
    pub fn results(&self) -> &Vec<TrialResult<StimulusType>> {
        &self.results
    }
}
