pub mod config;
pub mod state;
pub mod trial;
pub use config::ExperimentConfig;
pub use state::ExperimentStateMachine;
pub use trial::{Trial, TrialDurations, TrialTimestamps};
