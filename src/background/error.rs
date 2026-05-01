use std::fmt;

/// Errors reported by the job framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    /// The job panicked while executing.
    Panicked,
    /// The job completed without producing an output in once mode.
    MissingOutput,
}

/// Errors that can occur while submitting a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobSubmitError {
    /// The worker has already been shut down.
    Stopped,
}

impl fmt::Display for JobSubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => f.write_str("job worker has been stopped"),
        }
    }
}

impl std::error::Error for JobSubmitError {}
