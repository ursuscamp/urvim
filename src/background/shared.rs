use std::collections::BTreeMap;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};

use super::event::JobEvent;
use super::queue::JobQueues;
use super::token::{JobKind, JobToken};

/// Shared state visible to the worker and submitter.
#[derive(Debug)]
pub struct JobShared {
    pub(crate) queues: Mutex<JobQueues>,
    pub(crate) latest_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    pub(crate) aborted_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    pub(crate) available: Condvar,
    pub(crate) stopping: Arc<AtomicBool>,
    pub(crate) event_tx: Sender<JobEvent>,
}

impl JobShared {
    /// Creates shared job state.
    pub fn new(event_tx: Sender<JobEvent>) -> Self {
        Self {
            queues: Mutex::new(JobQueues::new()),
            latest_generations: Arc::new(Mutex::new(BTreeMap::new())),
            aborted_generations: Arc::new(Mutex::new(BTreeMap::new())),
            available: Condvar::new(),
            stopping: Arc::new(AtomicBool::new(false)),
            event_tx,
        }
    }

    /// Returns true when shutdown has been requested.
    pub fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// Marks a generation as aborted for the given job kind.
    pub fn abort_generation(&self, kind: JobKind, token: JobToken) {
        let mut generations = self.aborted_generations.lock().unwrap();
        generations
            .entry(kind)
            .and_modify(|generation| *generation = (*generation).max(token.generation()))
            .or_insert(token.generation());
    }

    /// Requests shutdown for the worker thread.
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }
}
