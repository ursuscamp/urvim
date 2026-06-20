use std::fmt;

/// Errors reported by the job framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    /// The job panicked while executing.
    Panicked,
    /// The job completed without producing an output in once mode.
    MissingOutput,
    /// The job reported an explicit failure message.
    Message(String),
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Panicked => f.write_str("job panicked"),
            Self::MissingOutput => f.write_str("job produced no output"),
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for JobError {}
