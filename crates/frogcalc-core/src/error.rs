use std::fmt;

#[derive(Clone, Debug)]
pub struct CalcError {
    pub message: String,
    pub pos: Option<usize>,
}

impl CalcError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            pos: None,
        }
    }

    pub fn at(message: impl Into<String>, pos: usize) -> Self {
        Self {
            message: message.into(),
            pos: Some(pos),
        }
    }
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CalcError {}

pub type CalcResult<T> = Result<T, CalcError>;
