use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, TryRecvError},
};
use std::thread::{self, JoinHandle};

use super::event::{JobEvent, JobSubmissionMode};
use super::queue::QueuedJob;
use super::shared::JobShared;
use super::token::{JobKind, JobToken};
use super::worker::worker_loop;
use super::{BackgroundJob, JobSubmitError};

/// Owns the background worker thread and event channel.
pub struct JobHandle {
    pub(crate) shared: Arc<JobShared>,
    event_rx: Mutex<Receiver<JobEvent>>,
    worker: Mutex<Option<JoinHandle<()>>>,
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
    /// Creates a new job handle with a dedicated worker thread.
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();
        let shared = Arc::new(JobShared::new(event_tx));
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("urvim-job-worker".to_string())
            .spawn(move || worker_loop(worker_shared))
            .expect("failed to spawn job worker thread");
        Self {
            shared,
            event_rx: Mutex::new(event_rx),
            worker: Mutex::new(Some(worker)),
        }
    }

    /// Submits a job using standard queueing semantics.
    pub fn submit<J>(&self, kind: JobKind, token: JobToken, job: J) -> Result<(), JobSubmitError>
    where
        J: Into<BackgroundJob>,
    {
        self.submit_internal(kind, token, JobSubmissionMode::Standard, job.into())
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
        self.submit_internal(kind, token, JobSubmissionMode::LatestOnly, job.into())
    }

    /// Polls the completion queue for the next job event.
    pub fn poll_event(&self) -> Option<JobEvent> {
        let receiver = self.event_rx.lock().unwrap();
        match receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Marks a job generation as aborted.
    pub fn abort_generation(&self, kind: JobKind, token: JobToken) {
        self.shared.abort_generation(kind, token);
    }

    /// Stops the worker thread and waits for it to exit.
    pub fn shutdown(&self) {
        self.shared.stop();
        self.shared.available.notify_all();
        if let Some(worker) = self.worker.lock().unwrap().take() {
            worker.join().ok();
        }
    }

    fn submit_internal(
        &self,
        kind: JobKind,
        token: JobToken,
        mode: JobSubmissionMode,
        job: BackgroundJob,
    ) -> Result<(), JobSubmitError> {
        if self.shared.is_stopping() {
            return Err(JobSubmitError::Stopped);
        }

        let mut queues = self.shared.queues.lock().unwrap();
        if self.shared.is_stopping() {
            return Err(JobSubmitError::Stopped);
        }

        {
            let mut generations = self.shared.latest_generations.lock().unwrap();
            generations.insert(kind.clone(), token.generation());
        }

        if matches!(mode, JobSubmissionMode::LatestOnly) {
            queues.discard_kind(&kind);
        }

        queues.push(QueuedJob::new(kind, token, job));
        self.shared.available.notify_one();
        Ok(())
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        self.shared.stop();
        self.shared.available.notify_all();

        #[cfg(test)]
        {
            return;
        }

        if let Some(worker) = self.worker.lock().unwrap().take() {
            worker.join().ok();
        }
    }
}
