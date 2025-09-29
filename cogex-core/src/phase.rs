/// Defines experiment phases and behavior
pub trait Phase: Copy + Clone + PartialEq + Send + Sync + std::fmt::Debug + Default {
    fn allows_input(&self) -> bool;
    fn requires_calibration(&self) -> bool;
    fn next(&self) -> Option<Self>;

    fn is_practice(&self) -> bool {
        false
    }
    fn is_experiment(&self) -> bool {
        false
    }

    fn is_welcome(&self) -> bool {
        false
    }
}

#[derive(Copy, Debug, Clone, PartialEq)]
pub enum StandardPhase {
    Welcome,
    Calibration,
    Practice,
    Experiment,
    Debrief,
}

impl Default for StandardPhase {
    fn default() -> Self {
        StandardPhase::Welcome
    }
}

impl Phase for StandardPhase {
    fn allows_input(&self) -> bool {
        !matches!(self, Self::Calibration)
    }
    fn requires_calibration(&self) -> bool {
        matches!(self, Self::Calibration)
    }
    fn next(&self) -> Option<Self> {
        use StandardPhase::*;
        Some(match self {
            Welcome => Calibration,
            Calibration => Practice,
            Practice => Experiment,
            Experiment => Debrief,
            Debrief => return None,
        })
    }

    fn is_practice(&self) -> bool {
        matches!(self, StandardPhase::Practice)
    }

    fn is_experiment(&self) -> bool {
        matches!(self, StandardPhase::Experiment)
    }

    fn is_welcome(&self) -> bool {
        matches!(self, StandardPhase::Welcome)
    }
}
