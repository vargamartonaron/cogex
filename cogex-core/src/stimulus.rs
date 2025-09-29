use cogex_cache::intern_text;
/// Defines stimuli and their render data
pub trait Stimulus: Clone + Send + Sync + std::fmt::Debug {
    fn cache_id(&self) -> usize;
    fn is_text(&self) -> bool;
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
        content: &'static str,
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
    fn cache_id(&self) -> usize {
        match self {
            StimulusType::Circle { .. } => 0,
            StimulusType::Rectangle { .. } => 1,
            StimulusType::Arrow { .. } => 2,
            StimulusType::Text { content, .. } => 3 + intern_text(content), // Add more variants here, ensuring unique IDs.
        }
    }

    fn is_text(&self) -> bool {
        matches!(self, StimulusType::Text { .. })
    }
}
