use std::collections::VecDeque;

use super::BackgroundJob;
use super::token::{JobKind, JobToken};

/// A queued background job.
#[derive(Debug)]
pub struct QueuedJob {
    pub(crate) kind: JobKind,
    pub(crate) token: JobToken,
    pub(crate) job: BackgroundJob,
}

impl QueuedJob {
    /// Creates a queued job entry.
    pub fn new(kind: JobKind, token: JobToken, job: BackgroundJob) -> Self {
        Self { kind, token, job }
    }
}

/// FIFO queue for pending background jobs.
#[derive(Debug, Default)]
pub struct JobQueues {
    jobs: VecDeque<QueuedJob>,
}

impl JobQueues {
    /// Creates an empty job queue.
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
        }
    }

    /// Pushes a job to the back of the queue.
    pub fn push(&mut self, job: QueuedJob) {
        self.jobs.push_back(job);
    }

    /// Removes queued jobs for the given kind.
    pub fn discard_kind(&mut self, kind: &JobKind) -> usize {
        let mut removed = 0;
        self.jobs.retain(|job| {
            let keep = job.kind != *kind;
            if !keep {
                removed += 1;
            }
            keep
        });
        removed
    }

    /// Pops the next queued job, if any.
    pub fn pop_next(&mut self) -> Option<QueuedJob> {
        self.jobs.pop_front()
    }
}
