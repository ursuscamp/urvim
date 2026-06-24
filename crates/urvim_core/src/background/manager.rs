use std::sync::atomic::{AtomicBool, Ordering};

use super::{BackgroundJob, JobEvent, JobKind, JobToken};
use urvim_background::JobSubmitError;

type InnerJobManager = urvim_background::JobManager<JobKind, BackgroundJob, JobEvent>;

/// Main-thread facade for submitting and consuming editor background jobs.
pub struct JobManager {
    inner: InnerJobManager,
    redraw_requested: AtomicBool,
}

impl std::fmt::Debug for JobManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobManager").finish_non_exhaustive()
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    /// Creates a new job manager with the default worker thread count.
    pub fn new() -> Self {
        #[cfg(test)]
        {
            return Self::with_workers(0);
        }

        #[cfg(not(test))]
        {
            Self::with_workers(4)
        }
    }

    /// Creates a new job manager with the given number of worker threads.
    pub fn with_workers(num_workers: usize) -> Self {
        Self {
            inner: InnerJobManager::with_workers(num_workers),
            redraw_requested: AtomicBool::new(false),
        }
    }

    /// Submits a job using standard queueing semantics.
    pub fn submit<J>(&self, kind: JobKind, token: JobToken, job: J) -> Result<(), JobSubmitError>
    where
        J: Into<BackgroundJob>,
    {
        self.inner.submit(kind, token, job.into())
    }

    /// Marks a job generation as aborted.
    pub fn abort_generation(&self, kind: JobKind, token: JobToken) {
        self.inner.abort_generation(kind, token);
    }

    /// Submits a job and discards older queued jobs for the same kind.
    pub fn submit_latest_only<J>(
        &self,
        kind: JobKind,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Into<BackgroundJob>,
    {
        self.inner.submit_latest_only(kind, token, job.into())
    }

    /// Polls the next job event.
    pub fn poll_event(&self) -> Option<JobEvent> {
        self.inner.poll_event()
    }

    /// Processes queued events and forwards accepted ones to the callback.
    pub fn process_events(&self, mut on_accepted: impl FnMut(JobEvent)) -> bool {
        let mut accepted_redraw = false;
        while let Some(event) = self.poll_event() {
            let kind = event.kind().clone();
            let token = event.token();
            if self.is_accepted(&kind, token) {
                match event {
                    JobEvent::Started { .. }
                    | JobEvent::Chunk { .. }
                    | JobEvent::Completed { .. } => {
                        accepted_redraw = true;
                        self.redraw_requested.store(true, Ordering::SeqCst);
                        on_accepted(event);
                    }
                    JobEvent::Failed { .. } => on_accepted(event),
                }
            }
        }
        accepted_redraw
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
    pub fn is_accepted(&self, kind: &JobKind, token: JobToken) -> bool {
        self.inner.is_accepted(kind, token)
    }

    /// Stops the worker thread.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }
}

/// Returns a process-wide shared `JobManager` for tests.
///
/// Tests that don't submit background jobs should use this instead of
/// `Arc::new(JobManager::new())` to avoid spawning unnecessary worker threads
/// when tests run in parallel.
#[cfg(test)]
pub fn shared_test_manager() -> std::sync::Arc<JobManager> {
    use std::sync::{Arc, OnceLock};
    static INSTANCE: OnceLock<Arc<JobManager>> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(JobManager::new())).clone()
}
