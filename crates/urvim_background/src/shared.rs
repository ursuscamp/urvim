use std::collections::BTreeMap;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::Sender,
};

use super::queue::JobQueues;
use crate::JobToken;

/// Shared state visible to the worker and submitter.
#[derive(Debug)]
pub struct JobShared<K, J, E>
where
    K: Ord,
{
    pub(crate) queues: Mutex<JobQueues<K, J>>,
    pub(crate) latest_generations: Arc<Mutex<BTreeMap<K, u64>>>,
    pub(crate) aborted_generations: Arc<Mutex<BTreeMap<K, u64>>>,
    pub(crate) available: Condvar,
    pub(crate) stopping: Arc<AtomicBool>,
    pub(crate) event_tx: Sender<E>,
}

impl<K, J, E> JobShared<K, J, E>
where
    K: Ord,
{
    /// Creates shared job state.
    pub fn new(event_tx: Sender<E>) -> Self {
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
    pub fn abort_generation(&self, kind: K, token: JobToken) {
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
