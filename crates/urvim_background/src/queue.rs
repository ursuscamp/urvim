use std::collections::VecDeque;

use crate::JobToken;

/// A queued background job.
#[derive(Debug)]
pub struct QueuedJob<K, J> {
    pub(crate) kind: K,
    pub(crate) token: JobToken,
    pub(crate) job: J,
}

impl<K, J> QueuedJob<K, J> {
    /// Creates a queued job entry.
    pub fn new(kind: K, token: JobToken, job: J) -> Self {
        Self { kind, token, job }
    }
}

/// FIFO queue for pending background jobs.
#[derive(Debug, Default)]
pub struct JobQueues<K, J> {
    jobs: VecDeque<QueuedJob<K, J>>,
}

impl<K, J> JobQueues<K, J>
where
    K: PartialEq,
{
    /// Creates an empty job queue.
    pub fn new() -> Self {
        Self {
            jobs: VecDeque::new(),
        }
    }

    /// Pushes a job to the back of the queue.
    pub fn push(&mut self, job: QueuedJob<K, J>) {
        self.jobs.push_back(job);
    }

    /// Removes queued jobs for the given kind.
    pub fn discard_kind(&mut self, kind: &K) -> usize {
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
    pub fn pop_next(&mut self) -> Option<QueuedJob<K, J>> {
        self.jobs.pop_front()
    }
}
