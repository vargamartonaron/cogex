/// Defines stimuli and their render data
pub trait Stimulus: Clone + Send + Sync + std::fmt::Debug {
    fn identifier(&self) -> &'static str;
}

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

impl Stimulus for StimulusType {
    fn identifier(&self) -> &'static str {
        match self {
            StimulusType::Circle { .. } => "circle",
            StimulusType::Rectangle { .. } => "rectangle",
            StimulusType::Arrow { .. } => "arrow",
            StimulusType::Text { .. } => "text",
        }
    }
}
