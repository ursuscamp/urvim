//! Internal job framework for deferred editor work.
//!
//! The job framework runs deferred work on a single serial background thread
//! and returns completed results to the main thread through a completion queue.
//! It is intentionally small so future deferred tasks can reuse the same
//! scheduling, cancellation, and shutdown behavior.

use smol_str::SmolStr;
use std::any::Any;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, Sender, TryRecvError},
};
use std::thread::{self, JoinHandle};

/// Priority tiers for queued jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    /// Higher-priority work that should run before background maintenance.
    Foreground,
    /// Lower-priority work that can wait behind foreground jobs.
    Background,
}

/// A generation token used to reject stale job results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobToken {
    generation: u64,
}

impl JobToken {
    /// Creates a new generation token.
    pub fn new(generation: u64) -> Self {
        Self { generation }
    }

    /// Returns the numeric generation value.
    pub fn generation(self) -> u64 {
        self.generation
    }
}

/// Controls how queued jobs behave when newer work supersedes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobSubmissionMode {
    /// Keep every queued job until it runs or is rejected after completion.
    Standard,
    /// Keep only the newest queued job for the same kind.
    LatestOnly,
}

/// Opaque identifier for the kind of work a job performs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobKind(SmolStr);

impl JobKind {
    /// Creates a new job kind from a descriptive label.
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self(name.into())
    }

    /// Returns the job kind label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for JobKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Shared context visible to a running job.
#[derive(Debug, Clone)]
pub struct JobContext {
    kind: JobKind,
    token: JobToken,
    stopping: Arc<AtomicBool>,
    latest_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
}

impl JobContext {
    fn new(
        kind: JobKind,
        token: JobToken,
        stopping: Arc<AtomicBool>,
        latest_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    ) -> Self {
        Self {
            kind,
            token,
            stopping,
            latest_generations,
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
}

/// A job that can run on the background worker thread.
pub trait Job: Send + 'static {
    /// The job's output type.
    type Output: Send + 'static;

    /// Runs the job and returns its output.
    fn run(self, context: &JobContext) -> Self::Output;
}

/// A completed or failed job result.
pub enum JobEvent {
    /// The job completed successfully.
    Completed {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The opaque output produced by the job.
        output: Box<dyn Any + Send>,
    },
    /// The job panicked while running.
    Failed {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The captured failure reason.
        error: JobError,
    },
}

impl JobEvent {
    /// Returns the job kind associated with this event.
    pub fn kind(&self) -> &JobKind {
        match self {
            Self::Completed { kind, .. } | Self::Failed { kind, .. } => kind,
        }
    }

    /// Returns the token associated with this event.
    pub fn token(&self) -> JobToken {
        match self {
            Self::Completed { token, .. } | Self::Failed { token, .. } => *token,
        }
    }

    /// Returns true when this event is a successful completion.
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// Attempts to downcast a successful completion into a typed output.
    pub fn into_completed_output<T: Send + 'static>(self) -> Result<(JobKind, JobToken, T), Self> {
        match self {
            Self::Completed {
                kind,
                token,
                output,
            } => match output.downcast::<T>() {
                Ok(output) => Ok((kind, token, *output)),
                Err(output) => Err(Self::Completed {
                    kind,
                    token,
                    output,
                }),
            },
            other => Err(other),
        }
    }
}

impl fmt::Debug for JobEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Completed { kind, token, .. } => f
                .debug_struct("JobEvent::Completed")
                .field("kind", kind)
                .field("token", token)
                .field("output", &"<opaque>")
                .finish(),
            Self::Failed { kind, token, error } => f
                .debug_struct("JobEvent::Failed")
                .field("kind", kind)
                .field("token", token)
                .field("error", error)
                .finish(),
        }
    }
}

/// Errors reported by the job framework.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    /// The job panicked while executing.
    Panicked,
}

/// Errors that can occur while submitting a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobSubmitError {
    /// The worker has already been shut down.
    Stopped,
}

impl fmt::Display for JobSubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => f.write_str("job worker has been stopped"),
        }
    }
}

impl std::error::Error for JobSubmitError {}

/// Handle used to submit work and poll completed jobs.
pub struct JobHandle {
    shared: Arc<JobShared>,
    completed_rx: Mutex<Receiver<JobEvent>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

/// Coordinates job submissions and completion handling on the main thread.
pub struct JobManager {
    handle: JobHandle,
    redraw_requested: AtomicBool,
}

impl fmt::Debug for JobManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobManager").finish_non_exhaustive()
    }
}

impl Default for JobManager {
    fn default() -> Self {
        Self::new()
    }
}

impl JobManager {
    /// Creates a new job manager and starts the worker thread.
    pub fn new() -> Self {
        Self {
            handle: JobHandle::new(),
            redraw_requested: AtomicBool::new(false),
        }
    }

    /// Submits a job using the standard queueing path.
    pub fn submit<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        match self.handle.submit(kind, priority, token, job) {
            Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Submits a job and keeps only the newest queued job for the same kind.
    pub fn submit_latest_only<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        match self.handle.submit_latest_only(kind, priority, token, job) {
            Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Polls the next raw job completion event.
    pub fn poll_completion(&self) -> Option<JobEvent> {
        self.handle.poll_completion()
    }

    /// Processes all completed jobs currently available on the queue.
    ///
    /// Accepted completions are passed to `on_accepted`. Stale completions are
    /// discarded and logged. Returns true when at least one accepted successful
    /// completion requested a redraw.
    pub fn process_completed(&self, mut on_accepted: impl FnMut(JobEvent)) -> bool {
        let mut accepted_redraw = false;

        while let Some(event) = self.poll_completion() {
            let kind = event.kind().clone();
            let token = event.token();

            if self.is_current(&kind, token) {
                match event {
                    JobEvent::Completed { output, .. } => {
                        tracing::debug!(
                            kind = %kind,
                            generation = token.generation(),
                            "accepted job completion"
                        );
                        accepted_redraw = true;
                        self.redraw_requested.store(true, Ordering::SeqCst);
                        on_accepted(JobEvent::Completed {
                            kind,
                            token,
                            output,
                        });
                    }
                    JobEvent::Failed { error, .. } => {
                        tracing::warn!(
                            kind = %kind,
                            generation = token.generation(),
                            ?error,
                            "job failed"
                        );
                        on_accepted(JobEvent::Failed { kind, token, error });
                    }
                }
            } else {
                tracing::debug!(
                    kind = %kind,
                    generation = token.generation(),
                    "discarded stale job completion"
                );
            }
        }

        accepted_redraw
    }

    /// Returns true when a redraw has been requested by accepted job completions.
    pub fn redraw_requested(&self) -> bool {
        self.redraw_requested.load(Ordering::SeqCst)
    }

    /// Returns and clears the redraw request latch.
    pub fn take_redraw_requested(&self) -> bool {
        self.redraw_requested.swap(false, Ordering::SeqCst)
    }

    /// Requests shutdown and waits for the worker thread to exit.
    pub fn shutdown(&self) {
        self.handle.shutdown();
    }

    fn is_current(&self, kind: &JobKind, token: JobToken) -> bool {
        self.handle.is_current(kind, token)
    }
}

impl fmt::Debug for JobHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JobHandle").finish_non_exhaustive()
    }
}

impl Default for JobHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl JobHandle {
    /// Creates a new job framework handle and starts the worker thread.
    pub fn new() -> Self {
        let (completed_tx, completed_rx) = mpsc::channel();
        let shared = Arc::new(JobShared::new(completed_tx));
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("urvim-job-worker".to_string())
            .spawn(move || worker_loop(worker_shared))
            .expect("failed to spawn job worker thread");

        Self {
            shared,
            completed_rx: Mutex::new(completed_rx),
            worker: Mutex::new(Some(worker)),
        }
    }

    /// Submits a job to the worker thread.
    pub fn submit<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        self.submit_internal(kind, priority, token, JobSubmissionMode::Standard, job)
    }

    /// Submits a job and keeps only the newest queued job for the same kind.
    pub fn submit_latest_only<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        self.submit_internal(kind, priority, token, JobSubmissionMode::LatestOnly, job)
    }

    /// Returns the next completed job event, if one is available.
    pub fn poll_completion(&self) -> Option<JobEvent> {
        let receiver = self.completed_rx.lock().unwrap();
        match receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    fn is_current(&self, kind: &JobKind, token: JobToken) -> bool {
        let generations = self.shared.latest_generations.lock().unwrap();
        generations
            .get(kind)
            .is_some_and(|generation| *generation == token.generation())
    }

    /// Requests shutdown and waits for the worker thread to exit.
    pub fn shutdown(&self) {
        self.shared.stop();
        self.shared.available.notify_all();

        let worker = self.worker.lock().unwrap().take();
        if let Some(worker) = worker {
            worker.join().ok();
        }
    }

    fn submit_internal<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        mode: JobSubmissionMode,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
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

        let removed_stale_jobs = if matches!(mode, JobSubmissionMode::LatestOnly) {
            queues.discard_kind(&kind)
        } else {
            0
        };

        tracing::debug!(
            kind = %kind,
            generation = token.generation(),
            priority = ?priority,
            mode = ?mode,
            removed_stale_jobs,
            "submitting job"
        );

        queues.push(QueuedJob::new(kind, priority, token, job));
        self.shared.available.notify_one();
        Ok(())
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        self.shared.stop();
        self.shared.available.notify_all();

        if let Some(worker) = self.worker.lock().unwrap().take() {
            worker.join().ok();
        }
    }
}

struct JobShared {
    queues: Mutex<JobQueues>,
    latest_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    available: Condvar,
    stopping: Arc<AtomicBool>,
    completed_tx: Sender<JobEvent>,
}

impl JobShared {
    fn new(completed_tx: Sender<JobEvent>) -> Self {
        Self {
            queues: Mutex::new(JobQueues::new()),
            latest_generations: Arc::new(Mutex::new(BTreeMap::new())),
            available: Condvar::new(),
            stopping: Arc::new(AtomicBool::new(false)),
            completed_tx,
        }
    }

    fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    fn is_current(&self, kind: &JobKind, token: JobToken) -> bool {
        let generations = self.latest_generations.lock().unwrap();
        generations
            .get(kind)
            .is_some_and(|generation| *generation == token.generation())
    }

    fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
    }
}

fn worker_loop(shared: Arc<JobShared>) {
    loop {
        let job = {
            let mut queues = shared.queues.lock().unwrap();
            loop {
                if let Some(job) = queues.pop_next() {
                    break job;
                }

                if shared.is_stopping() {
                    tracing::debug!("job worker stopping");
                    return;
                }

                queues = shared.available.wait(queues).unwrap();
            }
        };

        let kind = job.kind.clone();
        let token = job.token;
        if !shared.is_current(&kind, token) {
            tracing::debug!(
                kind = %kind,
                generation = token.generation(),
                "skipping stale job before execution"
            );
            continue;
        }

        let context = JobContext::new(
            kind.clone(),
            token,
            Arc::clone(&shared.stopping),
            Arc::clone(&shared.latest_generations),
        );
        let event = job.run(&context);
        if shared.completed_tx.send(event).is_err() {
            tracing::debug!(
                kind = %kind,
                generation = token.generation(),
                "dropping job completion because the receiver is gone"
            );
            return;
        }
    }
}

trait ErasedJob: Send {
    fn run(self: Box<Self>, context: &JobContext) -> JobEvent;
}

struct JobEnvelope<J: Job> {
    kind: JobKind,
    token: JobToken,
    job: J,
}

impl<J: Job> JobEnvelope<J> {
    fn new(kind: JobKind, token: JobToken, job: J) -> Self {
        Self { kind, token, job }
    }
}

impl<J: Job> ErasedJob for JobEnvelope<J> {
    fn run(self: Box<Self>, context: &JobContext) -> JobEvent {
        let Self { kind, token, job } = *self;
        let kind_for_event = kind.clone();
        let token_for_event = token;

        let output = catch_unwind(AssertUnwindSafe(|| job.run(context)));
        match output {
            Ok(output) => JobEvent::Completed {
                kind: kind_for_event,
                token: token_for_event,
                output: Box::new(output),
            },
            Err(_) => JobEvent::Failed {
                kind: kind_for_event,
                token: token_for_event,
                error: JobError::Panicked,
            },
        }
    }
}

struct QueuedJob {
    kind: JobKind,
    token: JobToken,
    priority: JobPriority,
    job: Box<dyn ErasedJob>,
}

impl QueuedJob {
    fn new<J: Job>(kind: JobKind, priority: JobPriority, token: JobToken, job: J) -> Self {
        let job = JobEnvelope::new(kind.clone(), token, job);
        Self {
            kind,
            token,
            priority,
            job: Box::new(job),
        }
    }

    fn run(self, context: &JobContext) -> JobEvent {
        self.job.run(context)
    }
}

struct JobQueues {
    foreground: VecDeque<QueuedJob>,
    background: VecDeque<QueuedJob>,
}

impl JobQueues {
    fn new() -> Self {
        Self {
            foreground: VecDeque::new(),
            background: VecDeque::new(),
        }
    }

    fn push(&mut self, job: QueuedJob) {
        match job.priority {
            JobPriority::Foreground => self.foreground.push_back(job),
            JobPriority::Background => self.background.push_back(job),
        }
    }

    fn discard_kind(&mut self, kind: &JobKind) -> usize {
        let mut removed = 0;
        self.foreground.retain(|job| {
            let keep = job.kind.as_str() != kind.as_str();
            if !keep {
                removed += 1;
            }
            keep
        });
        self.background.retain(|job| {
            let keep = job.kind.as_str() != kind.as_str();
            if !keep {
                removed += 1;
            }
            keep
        });
        removed
    }

    fn pop_next(&mut self) -> Option<QueuedJob> {
        if let Some(job) = self.foreground.pop_front() {
            return Some(job);
        }

        self.background.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    use std::thread;
    use std::time::{Duration, Instant};

    #[derive(Debug)]
    struct TraceJob {
        label: &'static str,
        trace: Arc<Mutex<Vec<&'static str>>>,
        started: Arc<AtomicUsize>,
    }

    impl Job for TraceJob {
        type Output = &'static str;

        fn run(self, _context: &JobContext) -> Self::Output {
            self.started.fetch_add(1, Ordering::SeqCst);
            self.trace.lock().unwrap().push(self.label);
            self.label
        }
    }

    fn wait_for_event(handle: &JobHandle) -> JobEvent {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event) = handle.poll_completion() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for job event");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn test_queue_prefers_foreground_jobs_and_preserves_fifo() {
        let mut queues = JobQueues::new();
        let trace = Arc::new(Mutex::new(Vec::new()));
        let started = Arc::new(AtomicUsize::new(0));

        queues.push(QueuedJob::new(
            JobKind::new("background-1"),
            JobPriority::Background,
            JobToken::new(1),
            TraceJob {
                label: "background-1",
                trace: Arc::clone(&trace),
                started: Arc::clone(&started),
            },
        ));
        queues.push(QueuedJob::new(
            JobKind::new("foreground-1"),
            JobPriority::Foreground,
            JobToken::new(2),
            TraceJob {
                label: "foreground-1",
                trace: Arc::clone(&trace),
                started: Arc::clone(&started),
            },
        ));
        queues.push(QueuedJob::new(
            JobKind::new("foreground-2"),
            JobPriority::Foreground,
            JobToken::new(3),
            TraceJob {
                label: "foreground-2",
                trace: Arc::clone(&trace),
                started: Arc::clone(&started),
            },
        ));
        queues.push(QueuedJob::new(
            JobKind::new("background-2"),
            JobPriority::Background,
            JobToken::new(4),
            TraceJob {
                label: "background-2",
                trace,
                started,
            },
        ));

        let first = queues.pop_next().expect("foreground job should exist");
        let second = queues.pop_next().expect("foreground job should exist");
        let third = queues.pop_next().expect("background job should exist");
        let fourth = queues.pop_next().expect("background job should exist");

        assert_eq!(first.kind.as_str(), "foreground-1");
        assert_eq!(second.kind.as_str(), "foreground-2");
        assert_eq!(third.kind.as_str(), "background-1");
        assert_eq!(fourth.kind.as_str(), "background-2");
    }

    #[test]
    fn test_handle_delivers_job_completions() {
        let handle = JobHandle::new();
        let trace = Arc::new(Mutex::new(Vec::new()));
        let started = Arc::new(AtomicUsize::new(0));
        let token = JobToken::new(42);

        handle
            .submit(
                JobKind::new("demo"),
                JobPriority::Foreground,
                token,
                TraceJob {
                    label: "demo",
                    trace: Arc::clone(&trace),
                    started: Arc::clone(&started),
                },
            )
            .expect("job submission should succeed");

        let event = wait_for_event(&handle);
        match event {
            JobEvent::Completed {
                kind,
                token: event_token,
                output,
            } => {
                assert_eq!(kind.as_str(), "demo");
                assert_eq!(event_token, token);
                let output = output
                    .downcast::<&'static str>()
                    .expect("job output should downcast");
                assert_eq!(*output, "demo");
            }
            other => panic!("expected completed event, got {:?}", other),
        }

        assert_eq!(started.load(Ordering::SeqCst), 1);
        assert_eq!(trace.lock().unwrap().as_slice(), &["demo"]);

        handle.shutdown();
    }

    #[test]
    fn test_handle_latest_only_jobs_skip_stale_queue_entries() {
        let handle = JobHandle::new();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_blocker = Arc::clone(&gate);
        let trace = Arc::new(Mutex::new(Vec::new()));
        let started = Arc::new(AtomicUsize::new(0));

        struct GateJob {
            gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
            label: &'static str,
            trace: Arc<Mutex<Vec<&'static str>>>,
            started: Arc<AtomicUsize>,
        }

        impl Job for GateJob {
            type Output = &'static str;

            fn run(self, _context: &JobContext) -> Self::Output {
                let (lock, cvar) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cvar.wait(open).unwrap();
                }
                self.started.fetch_add(1, Ordering::SeqCst);
                self.trace.lock().unwrap().push(self.label);
                self.label
            }
        }

        handle
            .submit(
                JobKind::new("blocker"),
                JobPriority::Foreground,
                JobToken::new(1),
                GateJob {
                    gate: gate_for_blocker,
                    label: "blocker",
                    trace: Arc::clone(&trace),
                    started: Arc::clone(&started),
                },
            )
            .expect("blocker job should submit");

        thread::sleep(Duration::from_millis(25));

        handle
            .submit_latest_only(
                JobKind::new("syntax"),
                JobPriority::Background,
                JobToken::new(2),
                TraceJob {
                    label: "old",
                    trace: Arc::clone(&trace),
                    started: Arc::clone(&started),
                },
            )
            .expect("old latest-only job should submit");

        handle
            .submit_latest_only(
                JobKind::new("syntax"),
                JobPriority::Background,
                JobToken::new(3),
                TraceJob {
                    label: "new",
                    trace: Arc::clone(&trace),
                    started: Arc::clone(&started),
                },
            )
            .expect("new latest-only job should submit");

        {
            let (lock, cvar) = &*gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cvar.notify_all();
        }

        let first = wait_for_event(&handle);
        let second = wait_for_event(&handle);

        let mut labels = Vec::new();
        for event in [first, second] {
            let (_kind, _token, output) = event
                .into_completed_output::<&'static str>()
                .expect("latest-only output should downcast");
            labels.push(output);
        }

        assert_eq!(labels.as_slice(), &["blocker", "new"]);
        assert_eq!(started.load(Ordering::SeqCst), 2);
        assert_eq!(trace.lock().unwrap().as_slice(), &["blocker", "new"]);

        handle.shutdown();
    }

    #[test]
    fn test_manager_discards_stale_job_completions() {
        let manager = JobManager::new();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_old = Arc::clone(&gate);
        let trace = Arc::new(Mutex::new(Vec::new()));

        struct GateJob {
            gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
            label: &'static str,
            trace: Arc<Mutex<Vec<&'static str>>>,
        }

        impl Job for GateJob {
            type Output = &'static str;

            fn run(self, _context: &JobContext) -> Self::Output {
                let (lock, cvar) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cvar.wait(open).unwrap();
                }
                self.trace.lock().unwrap().push(self.label);
                self.label
            }
        }

        manager
            .submit(
                JobKind::new("syntax"),
                JobPriority::Background,
                JobToken::new(1),
                GateJob {
                    gate: Arc::clone(&gate_for_old),
                    label: "old",
                    trace: Arc::clone(&trace),
                },
            )
            .expect("old job should submit");

        thread::sleep(Duration::from_millis(25));

        manager
            .submit(
                JobKind::new("syntax"),
                JobPriority::Background,
                JobToken::new(2),
                GateJob {
                    gate: Arc::clone(&gate),
                    label: "new",
                    trace: Arc::clone(&trace),
                },
            )
            .expect("new job should submit");

        {
            let (lock, cvar) = &*gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cvar.notify_all();
        }

        let mut accepted = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while accepted.len() < 1 {
            let _ = manager.process_completed(|event| accepted.push(event));
            assert!(
                Instant::now() < deadline,
                "timed out waiting for accepted job"
            );
            thread::sleep(Duration::from_millis(5));
        }

        assert!(accepted.iter().all(JobEvent::is_completed));
        assert_eq!(accepted.len(), 1);

        let event = accepted.pop().expect("one accepted event");
        let (kind, token, output) = event
            .into_completed_output::<&'static str>()
            .expect("accepted output should downcast");
        assert_eq!(kind.as_str(), "syntax");
        assert_eq!(token.generation(), 2);
        assert_eq!(output, "new");
        assert!(manager.redraw_requested());
        assert!(manager.take_redraw_requested());
        assert!(!manager.redraw_requested());
    }

    #[test]
    fn test_manager_reports_failed_current_job_and_does_not_request_redraw() {
        let manager = JobManager::new();

        struct PanicJob;

        impl Job for PanicJob {
            type Output = ();

            fn run(self, _context: &JobContext) -> Self::Output {
                panic!("boom");
            }
        }

        manager
            .submit(
                JobKind::new("panic"),
                JobPriority::Foreground,
                JobToken::new(9),
                PanicJob,
            )
            .expect("panic job should submit");

        let mut failures = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while failures.is_empty() {
            let _ = manager.process_completed(|event| failures.push(event));
            assert!(
                Instant::now() < deadline,
                "timed out waiting for failed job"
            );
            thread::sleep(Duration::from_millis(5));
        }

        assert!(matches!(&failures[0], JobEvent::Failed { .. }));
        assert!(!manager.redraw_requested());
    }

    #[test]
    fn test_job_context_reports_token_and_shutdown_state() {
        let handle = JobHandle::new();
        let observed = Arc::new(Mutex::new(None));
        let observed_clone = Arc::clone(&observed);

        struct ContextJob {
            observed: Arc<Mutex<Option<(String, JobToken, bool)>>>,
        }

        impl Job for ContextJob {
            type Output = ();

            fn run(self, context: &JobContext) -> Self::Output {
                *self.observed.lock().unwrap() = Some((
                    context.kind().as_str().to_string(),
                    context.token(),
                    context.is_stopping(),
                ));
            }
        }

        handle
            .submit(
                JobKind::new("context"),
                JobPriority::Background,
                JobToken::new(7),
                ContextJob {
                    observed: observed_clone,
                },
            )
            .expect("job submission should succeed");

        let event = wait_for_event(&handle);
        match event {
            JobEvent::Completed { kind, token, .. } => {
                assert_eq!(kind.as_str(), "context");
                assert_eq!(token.generation(), 7);
            }
            other => panic!("expected completed event, got {:?}", other),
        }

        let observed = observed.lock().unwrap();
        let (kind, token, is_stopping) = observed.as_ref().expect("context should be observed");
        assert_eq!(kind, "context");
        assert_eq!(token.generation(), 7);
        assert!(!is_stopping);

        handle.shutdown();
    }

    #[test]
    fn test_shutdown_is_idempotent_and_rejects_late_submissions() {
        let handle = JobHandle::new();
        handle.shutdown();
        handle.shutdown();

        let result = handle.submit(
            JobKind::new("late"),
            JobPriority::Background,
            JobToken::new(99),
            TraceJob {
                label: "late",
                trace: Arc::new(Mutex::new(Vec::new())),
                started: Arc::new(AtomicUsize::new(0)),
            },
        );

        assert_eq!(result, Err(JobSubmitError::Stopped));
    }
}
