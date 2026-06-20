use super::{BackgroundJob, JobEvent, JobKind, JobSubmitError, JobToken};

type InnerJobHandle = urvim_background::JobHandle<JobKind, BackgroundJob, JobEvent>;

/// Owns editor background worker threads and the event channel.
pub struct JobHandle {
    inner: InnerJobHandle,
}

impl Default for JobHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for JobHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandle").finish_non_exhaustive()
    }
}

impl JobHandle {
    /// Creates a new job handle with one worker thread.
    pub fn new() -> Self {
        Self {
            inner: InnerJobHandle::new(),
        }
    }

    /// Creates a new job handle with `num_workers` worker threads.
    pub fn with_workers(num_workers: usize) -> Self {
        Self {
            inner: InnerJobHandle::with_workers(num_workers),
        }
    }

    /// Submits a job using standard queueing semantics.
    pub fn submit<J>(&self, kind: JobKind, token: JobToken, job: J) -> Result<(), JobSubmitError>
    where
        J: Into<BackgroundJob>,
    {
        self.inner.submit(kind, token, job.into())
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

    /// Polls the completion queue for the next job event.
    pub fn poll_event(&self) -> Option<JobEvent> {
        self.inner.poll_event()
    }

    /// Marks a job generation as aborted.
    pub fn abort_generation(&self, kind: JobKind, token: JobToken) {
        self.inner.abort_generation(kind, token);
    }

    /// Stops all worker threads and waits for them to exit.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }

    /// Returns true when a job token still matches the latest submitted generation.
    pub fn is_accepted(&self, kind: &JobKind, token: JobToken) -> bool {
        self.inner.is_accepted(kind, token)
    }
}
