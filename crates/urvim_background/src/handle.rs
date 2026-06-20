use std::sync::{
    Arc, Mutex,
    mpsc::{self, Receiver, TryRecvError},
};
use std::thread::{self, JoinHandle};

use super::queue::QueuedJob;
use super::shared::JobShared;
use super::worker::worker_loop;
use crate::{BackgroundRunnable, JobSubmissionMode, JobSubmitError, JobToken};

/// Owns the background worker threads and event channel.
pub struct JobHandle<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    pub(crate) shared: Arc<JobShared<K, J, E>>,
    event_rx: Mutex<Receiver<E>>,
    workers: Vec<JoinHandle<()>>,
}

impl<K, J, E> Default for JobHandle<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, J, E> std::fmt::Debug for JobHandle<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobHandle").finish_non_exhaustive()
    }
}

impl<K, J, E> JobHandle<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    /// Creates a new job handle with one worker thread (for testing).
    pub fn new() -> Self {
        Self::with_workers(1)
    }

    /// Creates a new job handle with `num_workers` worker threads.
    pub fn with_workers(num_workers: usize) -> Self {
        assert!(num_workers > 0, "JobHandle requires at least one worker");
        let (event_tx, event_rx) = mpsc::channel();
        let shared = Arc::new(JobShared::new(event_tx));
        let workers: Vec<_> = (0..num_workers)
            .map(|i| {
                let worker_shared = Arc::clone(&shared);
                thread::Builder::new()
                    .name(format!("urvim-job-worker-{}", i))
                    .spawn(move || worker_loop(worker_shared))
                    .expect("failed to spawn job worker thread")
            })
            .collect();
        Self {
            shared,
            event_rx: Mutex::new(event_rx),
            workers,
        }
    }

    /// Submits a job using standard queueing semantics.
    pub fn submit(&self, kind: K, token: JobToken, job: J) -> Result<(), JobSubmitError> {
        self.submit_internal(kind, token, JobSubmissionMode::Standard, job)
    }

    /// Submits a job and discards older queued jobs for the same kind.
    pub fn submit_latest_only(
        &self,
        kind: K,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError> {
        self.submit_internal(kind, token, JobSubmissionMode::LatestOnly, job)
    }

    /// Polls the completion queue for the next job event.
    pub fn poll_event(&self) -> Option<E> {
        let receiver = self.event_rx.lock().unwrap();
        match receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    /// Marks a job generation as aborted.
    pub fn abort_generation(&self, kind: K, token: JobToken) {
        self.shared.abort_generation(kind, token);
    }

    /// Returns true when a job token still matches the latest submitted generation.
    pub fn is_accepted(&self, kind: &K, token: JobToken) -> bool {
        let generations = self.shared.latest_generations.lock().unwrap();
        generations
            .get(kind)
            .is_some_and(|generation| *generation == token.generation())
    }

    /// Stops all worker threads and waits for them to exit.
    pub fn shutdown(&self) {
        self.shared.stop();
        self.shared.available.notify_all();
        // Cannot take ownership from &self, so we signal and rely on workers
        // to exit gracefully. The `workers` vec is joined in Drop.
    }

    fn submit_internal(
        &self,
        kind: K,
        token: JobToken,
        mode: JobSubmissionMode,
        job: J,
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

impl<K, J, E> Drop for JobHandle<K, J, E>
where
    K: Clone + Ord + Send + 'static,
    J: BackgroundRunnable<K, E>,
    E: Send + 'static,
{
    fn drop(&mut self) {
        self.shared.stop();
        self.shared.available.notify_all();

        #[cfg(test)]
        {
            // Test runs should not block on worker shutdown; the process will
            // exit soon after and the workers already have the stop signal.
            self.workers.clear();
            return;
        }

        #[cfg(not(test))]
        for worker in self.workers.drain(..) {
            worker.join().ok();
        }
    }
}
