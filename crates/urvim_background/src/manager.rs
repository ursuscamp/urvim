use std::sync::atomic::{AtomicBool, Ordering};

use super::handle::JobHandle;
use crate::{BackgroundRunnable, JobSubmitError, JobToken};

/// Main-thread facade for submitting and consuming background jobs.
pub struct JobManager<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    handle: JobHandle<K, J, E>,
    redraw_requested: AtomicBool,
}

impl<K, J, E> std::fmt::Debug for JobManager<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobManager").finish_non_exhaustive()
    }
}

impl<K, J, E> Default for JobManager<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, J, E> JobManager<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    /// Creates a new job manager with the default worker thread count.
    pub fn new() -> Self {
        #[cfg(test)]
        {
            return Self::with_workers(1);
        }

        #[cfg(not(test))]
        {
            Self::with_workers(4)
        }
    }

    /// Creates a new job manager with the given number of worker threads.
    pub fn with_workers(num_workers: usize) -> Self {
        Self {
            handle: JobHandle::with_workers(num_workers),
            redraw_requested: AtomicBool::new(false),
        }
    }

    /// Submits a job using standard queueing semantics.
    pub fn submit(&self, kind: K, token: JobToken, job: J) -> Result<(), JobSubmitError> {
        self.handle.submit(kind, token, job)
    }

    /// Marks a job generation as aborted.
    pub fn abort_generation(&self, kind: K, token: JobToken) {
        self.handle.abort_generation(kind, token);
    }

    /// Submits a job and discards older queued jobs for the same kind.
    pub fn submit_latest_only(
        &self,
        kind: K,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError> {
        self.handle.submit_latest_only(kind, token, job)
    }

    /// Polls the next job event.
    pub fn poll_event(&self) -> Option<E> {
        self.handle.poll_event()
    }

    /// Marks that accepted work requested a redraw.
    pub fn mark_redraw_requested(&self) {
        self.redraw_requested.store(true, Ordering::SeqCst);
    }

    /// Returns true when a redraw has been requested by accepted work.
    pub fn redraw_requested(&self) -> bool {
        self.redraw_requested.load(Ordering::SeqCst)
    }

    /// Returns the redraw flag and clears it.
    pub fn take_redraw_requested(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::SeqCst)
    }

    /// Returns true when a job token still matches the latest submitted generation.
    pub fn is_accepted(&self, kind: &K, token: JobToken) -> bool {
        self.handle.is_accepted(kind, token)
    }

    /// Stops the worker thread.
    pub fn shutdown(&self) {
        self.handle.shutdown();
    }
}
