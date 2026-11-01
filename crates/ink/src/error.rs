/// Why a stroke can't be made from the given samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InkError {
    /// No sample was a finite point.
    Empty,
    /// More samples than [`crate::MAX_POINTS`]; the frontend splits long gestures.
    TooManyPoints { count: usize, max: usize },
    /// A coordinate is further from the page than any page could be ([`crate::MAX_COORD`]).
    OutOfRange,
}

impl InkError {
    /// A stable code for the engine's error JSON.
    pub fn code(&self) -> &'static str {
        match self {
            InkError::Empty => "ink_empty",
            InkError::TooManyPoints { .. } => "ink_too_many_points",
            InkError::OutOfRange => "ink_out_of_range",
        }
    }
}

impl std::fmt::Display for InkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InkError::Empty => f.write_str("the stroke has no usable points"),
            InkError::TooManyPoints { count, max } => {
                write!(
                    f,
                    "the stroke has {count} points; at most {max} are allowed"
                )
            }
            InkError::OutOfRange => f.write_str("a point is too far outside the page"),
        }
    }
}

impl std::error::Error for InkError {}
