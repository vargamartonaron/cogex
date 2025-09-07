// experiment.rs

use crate::timer::{HighPrecisionTimer, TimingInfo};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Calibration results struct
#[derive(Debug)]
pub struct Calibration {
    pub average_frame_time_ns: f64,
    pub jitter_ns: f64,
    pub min_frame_time_ns: f64,
    pub max_frame_time_ns: f64,
    pub effective_fps: f64,
}

impl Calibration {
    pub fn from_timing_info(info: &TimingInfo) -> Self {
        let avg_ms = info.average_frame_time / 1_000_000.0;
        let fps = if avg_ms > 0.0 { 1000.0 / avg_ms } else { 0.0 };
        Calibration {
            average_frame_time_ns: info.average_frame_time,
            jitter_ns: info.jitter,
            min_frame_time_ns: info.min_frame_time,
            max_frame_time_ns: info.max_frame_time,
            effective_fps: fps,
        }
    }
}

/// Experiment phases
#[derive(Debug, Clone, PartialEq)]
pub enum ExperimentPhase {
    Welcome,
    Calibration,
    Practice,
    Experiment,
    Debrief,
}

/// Trial states
#[derive(Debug, Clone, PartialEq)]
pub enum TrialState {
    Fixation,
    Stimulus,
    Response,
    Feedback,
    Complete,
}

/// Arrow directions
#[derive(Debug, Clone, PartialEq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Stimulus types
#[derive(Debug, Clone, PartialEq)]
pub enum StimulusType {
    Circle {
        radius: f32,
        color: [u8; 4],
    },
    Rectangle {
        width: f32,
        height: f32,
        color: [u8; 4],
    },
    Arrow {
        direction: ArrowDirection,
        size: f32,
        color: [u8; 4],
    },
    Text {
        content: String,
        size: f32,
        color: [u8; 4],
    },
}

/// Per trial data
#[derive(Debug)]
pub struct Trial {
    pub id: usize,
    pub stimulus: StimulusType,
    pub position: (f32, f32),

    pub fixation_ms: u64,
    pub stimulus_ms: u64,
    pub response_ms: u64,
    pub feedback_ms: u64,

    pub start_ns: u64,
    pub fixation_start_ns: u64,
    pub stimulus_start_ns: Option<u64>,
    pub response_ns: Option<u64>,

    pub state: TrialState,
}

/// Experiment configuration parameters
#[derive(Debug, Clone)]
pub struct ExperimentConfig {
    pub practice_trials: usize,
    pub experiment_trials: usize,

    pub fixation_range_ms: (u64, u64),
    pub stimulus_ms: u64,
    pub response_ms: u64,
    pub feedback_ms: u64,
    pub intertrial_ms: u64,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            practice_trials: 20,
            experiment_trials: 100,
            fixation_range_ms: (500, 1500),
            stimulus_ms: 200,
            response_ms: 2000,
            feedback_ms: 500,
            intertrial_ms: 1000,
        }
    }
}

/// Trial result data: to be saved/exported
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialResult {
    pub id: usize,
    pub stimulus_desc: String,
    pub reaction_ns: Option<u64>,
    pub correct: Option<bool>,
    pub timestamp_ns: u64,
}

/// Core experiment state
#[derive(Debug)]
pub struct ExperimentState {
    pub phase: ExperimentPhase,
    pub current_trial: Option<Trial>,

    pub trial_num: usize,
    pub practice_max: usize,
    pub experiment_max: usize,

    pub results: Vec<TrialResult>,

    pub config: ExperimentConfig,

    pub timer: HighPrecisionTimer,

    pub calibration: Option<Calibration>,
    pub calibrated: bool,
    pub safe_margin_ns: u64,
}

impl ExperimentState {
    pub fn new() -> Self {
        let config = ExperimentConfig::default();
        Self {
            phase: ExperimentPhase::Welcome,
            current_trial: None,
            trial_num: 0,
            practice_max: config.practice_trials,
            experiment_max: config.experiment_trials,
            results: Vec::new(),
            config,
            timer: HighPrecisionTimer::new(),
            calibration: None,
            calibrated: false,
            safe_margin_ns: 0,
        }
    }

    pub fn advance_calibration(&mut self) {
        println!("Starting calibration...");
        self.phase = ExperimentPhase::Calibration;
        self.timer = HighPrecisionTimer::new();
        self.calibrated = false;
        self.calibration = None;
        self.trial_num = 0;
        self.current_trial = None;
    }

    pub fn advance_practice(&mut self) {
        println!("Starting practice trials...");
        self.phase = ExperimentPhase::Practice;
        self.trial_num = 0;
        self.start_trial();
    }

    pub fn advance_experiment(&mut self) {
        println!("Starting experiment trials...");
        self.phase = ExperimentPhase::Experiment;
        self.trial_num = 0;
        self.start_trial();
    }

    pub fn advance_debrief(&mut self) {
        println!("Starting debrief phase...");
        self.phase = ExperimentPhase::Debrief;
        self.current_trial = None;
        self.analyze_results();
    }

    pub fn calibrated(&self) -> bool {
        self.calibrated
    }

    pub fn practice_done(&self) -> bool {
        self.phase == ExperimentPhase::Practice && self.trial_num >= self.practice_max
    }

    pub fn experiment_done(&self) -> bool {
        self.phase == ExperimentPhase::Experiment && self.trial_num >= self.experiment_max
    }

    pub fn apply_calibration(&mut self) {
        let info = self.timer.get_info();
        let calib = Calibration::from_timing_info(&info);
        println!(
            "Calibration results: {:.3} ms/frame, {:.1} Hz, jitter {:.3} ms",
            calib.average_frame_time_ns / 1_000_000.0,
            calib.effective_fps,
            calib.jitter_ns / 1_000_000.0,
        );
        self.calibration = Some(calib);
        self.safe_margin_ns = (self.calibration.as_ref().unwrap().jitter_ns * 3.0) as u64;
        // add margin (ms) to stimulus duration for safety
        self.config.stimulus_ms += (self.safe_margin_ns / 1_000_000);
        self.calibrated = true;
    }

    pub fn start_trial(&mut self) {
        use rand::thread_rng;
        let mut rng = thread_rng();

        let id = self.trial_num;
        let stim = self.generate_stimulus();
        let pos = self.generate_position();

        let fixation =
            rng.gen_range(self.config.fixation_range_ms.0..=self.config.fixation_range_ms.1);

        let now_ns = self.timer.get_timestamp();

        let trial = Trial {
            id,
            stimulus: stim,
            position: pos,
            fixation_ms: fixation,
            stimulus_ms: self.config.stimulus_ms,
            response_ms: self.config.response_ms,
            feedback_ms: self.config.feedback_ms,
            start_ns: now_ns,
            fixation_start_ns: now_ns,
            stimulus_start_ns: None,
            response_ns: None,
            state: TrialState::Fixation,
        };

        self.current_trial = Some(trial);
        println!("Trial {} started at {} ns", id, now_ns);
    }

    pub fn update_trial(&mut self) {
        if !self.calibrated {
            return;
        }

        let now_ns = self.timer.get_timestamp();

        if let Some(trial) = &mut self.current_trial {
            match trial.state {
                TrialState::Fixation => {
                    if now_ns - trial.fixation_start_ns >= trial.fixation_ms * 1_000_000 {
                        trial.state = TrialState::Stimulus;
                        trial.stimulus_start_ns = Some(now_ns);
                        println!("Stimulus started at {}", now_ns);
                    }
                }
                TrialState::Stimulus => {
                    let dur_ns = trial.stimulus_ms * 1_000_000 + self.safe_margin_ns;
                    if let Some(start_ns) = trial.stimulus_start_ns {
                        if now_ns - start_ns >= dur_ns {
                            trial.state = TrialState::Response;
                            println!("Response window opened at {}", now_ns);
                        }
                    }
                }
                TrialState::Response => {
                    let total_ns =
                        (trial.stimulus_ms + trial.response_ms) * 1_000_000 + self.safe_margin_ns;
                    if let Some(start_ns) = trial.stimulus_start_ns {
                        if now_ns - start_ns >= total_ns {
                            self.complete_trial(None);
                        }
                    }
                }
                TrialState::Feedback => {
                    let total_ns = (trial.fixation_ms
                        + trial.stimulus_ms
                        + trial.response_ms
                        + trial.feedback_ms)
                        * 1_000_000
                        + self.safe_margin_ns;
                    if now_ns - trial.start_ns >= total_ns {
                        trial.state = TrialState::Complete;
                        self.next_trial();
                    }
                }
                TrialState::Complete => {}
            }
        }
    }

    pub fn record_response(&mut self) {
        if let Some(trial) = &mut self.current_trial {
            if trial.state == TrialState::Response {
                let now_ns = self.timer.get_timestamp();
                trial.response_ns = Some(now_ns);
                trial.state = TrialState::Feedback;
                let rt = now_ns - trial.stimulus_start_ns.unwrap_or(now_ns);
                println!(
                    "Response recorded at {}, RT = {:.3} ms",
                    now_ns,
                    rt as f64 / 1_000_000.0
                );
                self.complete_trial(Some(now_ns));
            }
        }
    }

    fn complete_trial(&mut self, timestamp: Option<u64>) {
        if let Some(trial) = &self.current_trial {
            let reaction_ns = trial
                .response_ns
                .map(|r| r - trial.stimulus_start_ns.unwrap_or(r));
            let correct = reaction_ns.is_some();

            let result = TrialResult {
                id: trial.id,
                stimulus_desc: format!("{:?}", trial.stimulus),
                reaction_ns,
                correct: Some(correct),
                timestamp_ns: timestamp.unwrap_or(0),
            };

            self.results.push(result);
        }
    }

    fn next_trial(&mut self) {
        self.trial_num += 1;
        self.current_trial = None;

        self.timer
            .high_precision_sleep(Duration::from_micros(self.config.intertrial_ms * 1000));

        if self.phase == ExperimentPhase::Practice && self.trial_num >= self.practice_max {
            self.advance_experiment();
        } else if self.phase == ExperimentPhase::Experiment && self.trial_num >= self.experiment_max
        {
            self.advance_debrief();
        } else {
            self.start_trial();
        }
    }

    pub fn analyze_results(&self) {
        if self.results.is_empty() {
            return;
        }
        let valid_results: Vec<_> = self
            .results
            .iter()
            .filter(|r| r.reaction_ns.is_some())
            .collect();

        let rate = valid_results.len() as f64 / self.results.len() as f64 * 100.0;
        let times: Vec<f64> = valid_results
            .iter()
            .map(|r| r.reaction_ns.unwrap() as f64 / 1_000_000.0)
            .collect();

        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        println!("Experiment Results:");
        println!(
            "Trials: {}, Response rate: {:.1}%",
            self.results.len(),
            rate
        );
        println!(
            "Reaction times: mean {:.3} ms, min {:.3} ms, max {:.3} ms",
            mean, min, max
        );

        let file =
            std::fs::File::create("experiment_results.json").expect("Cannot create result file");
        serde_json::to_writer_pretty(file, &self.results).expect("Failed to write results");
        println!("Results saved to experiment_results.json");
    }

    fn generate_stimulus(&self) -> StimulusType {
        use rand::thread_rng;
        let mut rng = thread_rng();

        match rng.gen_range(0..4) {
            0 => StimulusType::Circle {
                radius: rng.gen_range(20.0..50.0),
                color: [255, 0, 0, 255],
            },
            1 => StimulusType::Rectangle {
                width: rng.gen_range(40.0..80.0),
                height: rng.gen_range(40.0..80.0),
                color: [0, 255, 0, 255],
            },
            2 => StimulusType::Arrow {
                direction: match rng.gen_range(0..4) {
                    0 => ArrowDirection::Up,
                    1 => ArrowDirection::Down,
                    2 => ArrowDirection::Left,
                    _ => ArrowDirection::Right,
                },
                size: rng.gen_range(30.0..60.0),
                color: [0, 0, 255, 255],
            },
            _ => StimulusType::Text {
                content: ["GO", "STOP", "WAIT"][rng.gen_range(0..3)].to_string(),
                size: rng.gen_range(24.0..36.0),
                color: [255, 255, 255, 255],
            },
        }
    }

    fn generate_position(&self) -> (f32, f32) {
        let mut rng = rand::thread_rng();
        (rng.gen_range(100.0..700.0), rng.gen_range(100.0..500.0))
    }
}

impl Trial {
    pub fn stimulus_description(&self) -> String {
        format!("{:?}", self.stimulus)
    }
}
