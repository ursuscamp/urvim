use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use super::token::{JobKind, JobToken};

/// Shared context visible to a running job.
#[derive(Debug, Clone)]
pub struct JobContext {
    kind: JobKind,
    token: JobToken,
    stopping: Arc<AtomicBool>,
    latest_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    aborted_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
}

impl JobContext {
    pub(crate) fn new(
        kind: JobKind,
        token: JobToken,
        stopping: Arc<AtomicBool>,
        latest_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
        aborted_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    ) -> Self {
        Self {
            kind,
            token,
            stopping,
            latest_generations,
            aborted_generations,
        }
    }

    /// Returns the job kind associated with this execution.
    pub fn kind(&self) -> &JobKind {
        &self.kind
    }
    /// Returns the token associated with this execution.
    pub fn token(&self) -> JobToken {
        self.token
    }
    /// Returns true when shutdown has been requested.
    pub fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }
    /// Returns true when this job still matches the latest submitted generation for its kind.
    pub fn is_current(&self) -> bool {
        let generations = self.latest_generations.lock().unwrap();
        generations
            .get(&self.kind)
            .is_some_and(|generation| *generation == self.token.generation())
    }
    /// Returns true when this job generation has been explicitly aborted.
    pub fn is_aborted(&self) -> bool {
        let generations = self.aborted_generations.lock().unwrap();
        generations
            .get(&self.kind)
            .is_some_and(|generation| *generation >= self.token.generation())
    }
}
