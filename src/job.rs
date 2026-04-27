//! Internal job framework for deferred editor work.
//!
//! The job framework runs deferred work on a single serial background thread
//! and returns job events to the main thread through a completion queue.
//! It is intentionally small so future deferred tasks can reuse the same
//! scheduling, cancellation, and shutdown behavior.

use crate::buffer::BufferCacheRefreshResult;
use crate::ui::file_picker::FilePickerItem;
use smol_str::SmolStr;
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
    /// Keep every queued job until it runs or is superseded.
    Standard,
    /// Keep only the newest queued job for the same kind.
    LatestOnly,
}

/// Controls whether a job delivers one final output or a stream of chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobDelivery {
    /// The job produces one final output.
    Once,
    /// The job produces incremental output chunks.
    Streaming,
}

/// Payload carried by job events.
#[derive(Debug, Clone)]
pub enum JobPayload {
    /// Buffer syntax cache refresh result.
    BufferCacheRefresh(BufferCacheRefreshResult),
    /// File picker search chunk.
    FilePickerChunk(Vec<FilePickerItem>),
    /// Empty payload for jobs that only signal completion.
    Unit,
    #[cfg(test)]
    /// Test-only text payload used to exercise the job pipeline.
    Test(SmolStr),
}

impl From<BufferCacheRefreshResult> for JobPayload {
    fn from(value: BufferCacheRefreshResult) -> Self {
        Self::BufferCacheRefresh(value)
    }
}

impl From<Vec<FilePickerItem>> for JobPayload {
    fn from(value: Vec<FilePickerItem>) -> Self {
        Self::FilePickerChunk(value)
    }
}

#[cfg(test)]
impl From<&'static str> for JobPayload {
    fn from(value: &'static str) -> Self {
        Self::Test(SmolStr::new(value))
    }
}

impl From<()> for JobPayload {
    fn from(_: ()) -> Self {
        Self::Unit
    }
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
    aborted_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
}

impl JobContext {
    fn new(
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

/// A job that can run on the background worker thread.
pub trait Job: Send + 'static {
    /// The job's output type.
    type Output: Into<JobPayload> + Send + 'static;

    /// Runs the job and emits its output in order.
    fn run(self, context: &JobContext, emit: &mut dyn FnMut(Self::Output));
}

/// A job lifecycle event.
pub enum JobEvent {
    /// The job started running.
    Started {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
    },
    /// The job produced a chunk of output.
    Chunk {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The chunk payload produced by the job.
        payload: JobPayload,
    },
    /// The job completed successfully.
    Completed {
        /// The kind supplied at submission time.
        kind: JobKind,
        /// The token supplied at submission time.
        token: JobToken,
        /// The output produced by the job, if any.
        payload: Option<JobPayload>,
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
            Self::Started { kind, .. }
            | Self::Chunk { kind, .. }
            | Self::Completed { kind, .. }
            | Self::Failed { kind, .. } => kind,
        }
    }

    /// Returns the token associated with this event.
    pub fn token(&self) -> JobToken {
        match self {
            Self::Started { token, .. }
            | Self::Chunk { token, .. }
            | Self::Completed { token, .. }
            | Self::Failed { token, .. } => *token,
        }
    }

    /// Returns true when this event is a successful lifecycle event.
    pub fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// Returns true when this event marks the start of a job.
    pub fn is_started(&self) -> bool {
        matches!(self, Self::Started { .. })
    }

    /// Returns true when this event carries streamed output.
    pub fn is_chunk(&self) -> bool {
        matches!(self, Self::Chunk { .. })
    }

    /// Returns true when this event is a terminal completion.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed { .. } | Self::Failed { .. })
    }

    /// Returns the chunk payload if this event is a chunk.
    pub fn into_chunk_payload(self) -> Result<(JobKind, JobToken, JobPayload), Self> {
        match self {
            Self::Chunk {
                kind,
                token,
                payload,
            } => Ok((kind, token, payload)),
            other => Err(other),
        }
    }

    /// Returns the completion payload if this event is a terminal completion.
    pub fn into_completed_payload(self) -> Result<(JobKind, JobToken, Option<JobPayload>), Self> {
        match self {
            Self::Completed {
                kind,
                token,
                payload,
            } => Ok((kind, token, payload)),
            other => Err(other),
        }
    }
}

impl fmt::Debug for JobEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Started { kind, token } => f
                .debug_struct("JobEvent::Started")
                .field("kind", kind)
                .field("token", token)
                .finish(),
            Self::Chunk { kind, token, .. } => f
                .debug_struct("JobEvent::Chunk")
                .field("kind", kind)
                .field("token", token)
                .field("payload", &"<opaque>")
                .finish(),
            Self::Completed { kind, token, .. } => f
                .debug_struct("JobEvent::Completed")
                .field("kind", kind)
                .field("token", token)
                .field("payload", &"<opaque>")
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
    /// The job completed without producing an output in once mode.
    MissingOutput,
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

/// Handle used to submit work and poll job events.
pub struct JobHandle {
    shared: Arc<JobShared>,
    event_rx: Mutex<Receiver<JobEvent>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

/// Coordinates job submissions and event handling on the main thread.
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
        delivery: JobDelivery,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        match self.handle.submit(kind, priority, token, delivery, job) {
            Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Marks a generation as aborted for best-effort cancellation.
    pub fn abort_generation(&self, kind: JobKind, token: JobToken) {
        self.handle.abort_generation(kind, token);
    }

    /// Submits a job and keeps only the newest queued job for the same kind.
    pub fn submit_latest_only<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        delivery: JobDelivery,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        match self
            .handle
            .submit_latest_only(kind, priority, token, delivery, job)
        {
            Ok(()) => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Polls the next raw job event.
    pub fn poll_event(&self) -> Option<JobEvent> {
        self.handle.poll_event()
    }

    /// Processes all jobs currently available on the queue.
    ///
    /// Accepted events are passed to `on_accepted`. Stale events are discarded
    /// and logged. Returns true when at least one accepted successful event
    /// requested a redraw.
    pub fn process_events(&self, mut on_accepted: impl FnMut(JobEvent)) -> bool {
        let mut accepted_redraw = false;

        while let Some(event) = self.poll_event() {
            let kind = event.kind().clone();
            let token = event.token();

            if self.is_accepted(&kind, token) {
                match event {
                    JobEvent::Started { .. } => {
                        tracing::debug!(
                            kind = %kind,
                            generation = token.generation(),
                            "accepted job start"
                        );
                        accepted_redraw = true;
                        self.redraw_requested.store(true, Ordering::SeqCst);
                        on_accepted(JobEvent::Started { kind, token });
                    }
                    JobEvent::Chunk { payload, .. } => {
                        tracing::debug!(
                            kind = %kind,
                            generation = token.generation(),
                            "accepted job chunk"
                        );
                        accepted_redraw = true;
                        self.redraw_requested.store(true, Ordering::SeqCst);
                        on_accepted(JobEvent::Chunk {
                            kind,
                            token,
                            payload,
                        });
                    }
                    JobEvent::Completed { payload, .. } => {
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
                            payload,
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
                    "discarded stale job event"
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

    fn is_accepted(&self, kind: &JobKind, token: JobToken) -> bool {
        self.handle.is_accepted(kind, token)
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

    /// Submits a job to the worker thread.
    pub fn submit<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        delivery: JobDelivery,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        self.submit_internal(
            kind,
            priority,
            token,
            JobSubmissionMode::Standard,
            delivery,
            job,
        )
    }

    /// Submits a job and keeps only the newest queued job for the same kind.
    pub fn submit_latest_only<J>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        delivery: JobDelivery,
        job: J,
    ) -> Result<(), JobSubmitError>
    where
        J: Job,
    {
        self.submit_internal(
            kind,
            priority,
            token,
            JobSubmissionMode::LatestOnly,
            delivery,
            job,
        )
    }

    /// Returns the next job event, if one is available.
    pub fn poll_event(&self) -> Option<JobEvent> {
        let receiver = self.event_rx.lock().unwrap();
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

    fn is_aborted(&self, kind: &JobKind, token: JobToken) -> bool {
        let generations = self.shared.aborted_generations.lock().unwrap();
        generations
            .get(kind)
            .is_some_and(|generation| *generation >= token.generation())
    }

    fn is_accepted(&self, kind: &JobKind, token: JobToken) -> bool {
        self.is_current(kind, token) && !self.is_aborted(kind, token)
    }

    /// Marks a generation as aborted.
    pub fn abort_generation(&self, kind: JobKind, token: JobToken) {
        self.shared.abort_generation(kind, token);
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
        delivery: JobDelivery,
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
            delivery = ?delivery,
            removed_stale_jobs,
            "submitting job"
        );

        queues.push(QueuedJob::new(kind, priority, token, delivery, job));
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
    aborted_generations: Arc<Mutex<BTreeMap<JobKind, u64>>>,
    available: Condvar,
    stopping: Arc<AtomicBool>,
    event_tx: Sender<JobEvent>,
}

impl JobShared {
    fn new(event_tx: Sender<JobEvent>) -> Self {
        Self {
            queues: Mutex::new(JobQueues::new()),
            latest_generations: Arc::new(Mutex::new(BTreeMap::new())),
            aborted_generations: Arc::new(Mutex::new(BTreeMap::new())),
            available: Condvar::new(),
            stopping: Arc::new(AtomicBool::new(false)),
            event_tx,
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

    fn is_aborted(&self, kind: &JobKind, token: JobToken) -> bool {
        let generations = self.aborted_generations.lock().unwrap();
        generations
            .get(kind)
            .is_some_and(|generation| *generation >= token.generation())
    }

    fn abort_generation(&self, kind: JobKind, token: JobToken) {
        let mut generations = self.aborted_generations.lock().unwrap();
        generations
            .entry(kind)
            .and_modify(|generation| {
                *generation = (*generation).max(token.generation());
            })
            .or_insert(token.generation());
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
        if !shared.is_current(&kind, token) || shared.is_aborted(&kind, token) {
            tracing::debug!(
                kind = %kind,
                generation = token.generation(),
                "skipping stale or aborted job before execution"
            );
            continue;
        }

        let context = JobContext::new(
            kind.clone(),
            token,
            Arc::clone(&shared.stopping),
            Arc::clone(&shared.latest_generations),
            Arc::clone(&shared.aborted_generations),
        );
        job.job.run(&context, job.delivery, &shared.event_tx);
    }
}

trait ErasedJob: Send {
    fn run(
        self: Box<Self>,
        context: &JobContext,
        delivery: JobDelivery,
        event_tx: &Sender<JobEvent>,
    );
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
    fn run(
        self: Box<Self>,
        context: &JobContext,
        delivery: JobDelivery,
        event_tx: &Sender<JobEvent>,
    ) {
        let Self { kind, token, job } = *self;
        let kind_for_event = kind.clone();
        let token_for_event = token;

        if matches!(delivery, JobDelivery::Streaming)
            && event_tx
                .send(JobEvent::Started {
                    kind: kind_for_event.clone(),
                    token: token_for_event,
                })
                .is_err()
        {
            return;
        }

        let output = catch_unwind(AssertUnwindSafe(|| {
            let mut captured_output: Option<JobPayload> = None;
            let mut emit = |event: J::Output| match delivery {
                JobDelivery::Once => {
                    if captured_output.is_none() {
                        captured_output = Some(event.into());
                    } else {
                        tracing::warn!(
                            kind = %kind_for_event,
                            generation = token_for_event.generation(),
                            "job emitted multiple outputs in once mode"
                        );
                    }
                }
                JobDelivery::Streaming => {
                    if event_tx
                        .send(JobEvent::Chunk {
                            kind: kind_for_event.clone(),
                            token: token_for_event,
                            payload: event.into(),
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            };

            job.run(context, &mut emit);
            captured_output
        }));

        match output {
            Ok(Some(output)) => {
                if event_tx
                    .send(JobEvent::Completed {
                        kind: kind_for_event,
                        token: token_for_event,
                        payload: Some(output),
                    })
                    .is_err()
                {
                    tracing::debug!(
                        kind = %kind,
                        generation = token.generation(),
                        "dropping job completion because the receiver is gone"
                    );
                }
            }
            Ok(None) => match delivery {
                JobDelivery::Once => {
                    let _ = event_tx.send(JobEvent::Failed {
                        kind: kind_for_event,
                        token: token_for_event,
                        error: JobError::MissingOutput,
                    });
                }
                JobDelivery::Streaming => {
                    let _ = event_tx.send(JobEvent::Completed {
                        kind: kind_for_event,
                        token: token_for_event,
                        payload: None,
                    });
                }
            },
            Err(_) => {
                let _ = event_tx.send(JobEvent::Failed {
                    kind: kind_for_event,
                    token: token_for_event,
                    error: JobError::Panicked,
                });
            }
        }
    }
}

struct QueuedJob {
    kind: JobKind,
    token: JobToken,
    priority: JobPriority,
    delivery: JobDelivery,
    job: Box<dyn ErasedJob>,
}

impl QueuedJob {
    fn new<J: Job>(
        kind: JobKind,
        priority: JobPriority,
        token: JobToken,
        delivery: JobDelivery,
        job: J,
    ) -> Self {
        let job = JobEnvelope::new(kind.clone(), token, job);
        Self {
            kind,
            token,
            priority,
            delivery,
            job: Box::new(job),
        }
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
        atomic::{AtomicBool, AtomicUsize, Ordering},
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

        fn run(self, _context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
            self.started.fetch_add(1, Ordering::SeqCst);
            self.trace.lock().unwrap().push(self.label);
            emit(self.label);
        }
    }

    fn wait_for_event(handle: &JobHandle) -> JobEvent {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event) = handle.poll_event() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for job event");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[derive(Debug)]
    struct TraceStreamingJob {
        trace: Arc<Mutex<Vec<&'static str>>>,
    }

    impl Job for TraceStreamingJob {
        type Output = &'static str;

        fn run(self, _context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
            self.trace.lock().unwrap().push("run");
            emit("chunk-1");
            emit("chunk-2");
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
            JobDelivery::Once,
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
            JobDelivery::Once,
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
            JobDelivery::Once,
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
            JobDelivery::Once,
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
                JobDelivery::Once,
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
                payload: Some(JobPayload::Test(output)),
            } => {
                assert_eq!(kind.as_str(), "demo");
                assert_eq!(event_token, token);
                assert_eq!(output, "demo");
            }
            other => panic!("expected completed event, got {:?}", other),
        }

        assert_eq!(started.load(Ordering::SeqCst), 1);
        assert_eq!(trace.lock().unwrap().as_slice(), &["demo"]);

        handle.shutdown();
    }

    #[test]
    fn test_handle_delivers_streaming_job_events_in_order() {
        let handle = JobHandle::new();
        let trace = Arc::new(Mutex::new(Vec::new()));
        let token = JobToken::new(7);

        handle
            .submit(
                JobKind::new("stream"),
                JobPriority::Foreground,
                token,
                JobDelivery::Streaming,
                TraceStreamingJob {
                    trace: Arc::clone(&trace),
                },
            )
            .expect("streaming job should submit");

        let mut events = Vec::new();
        while events.len() < 4 {
            events.push(wait_for_event(&handle));
        }

        match &events[0] {
            JobEvent::Started {
                kind,
                token: event_token,
            } => {
                assert_eq!(kind.as_str(), "stream");
                assert_eq!(*event_token, token);
            }
            other => panic!("expected started event, got {:?}", other),
        }

        let chunks: Vec<SmolStr> = events
            .iter()
            .filter_map(|event| match event {
                JobEvent::Chunk {
                    payload: JobPayload::Test(output),
                    ..
                } => Some(output.clone()),
                _ => None,
            })
            .collect();

        assert_eq!(
            chunks.as_slice(),
            &[SmolStr::new("chunk-1"), SmolStr::new("chunk-2")]
        );

        match &events[3] {
            JobEvent::Completed {
                kind,
                token: event_token,
                ..
            } => {
                assert_eq!(kind.as_str(), "stream");
                assert_eq!(*event_token, token);
            }
            other => panic!("expected completed event, got {:?}", other),
        }

        assert_eq!(trace.lock().unwrap().as_slice(), &["run"]);

        handle.shutdown();
    }

    #[test]
    fn test_streaming_jobs_stop_after_abort_and_discard_stale_events() {
        let manager = JobManager::new();
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let observed_abort = Arc::new(AtomicBool::new(false));
        let kind = JobKind::new("stream");
        let old_token = JobToken::new(1);
        let new_token = JobToken::new(2);

        struct GateStreamingJob {
            gate: Arc<(Mutex<bool>, std::sync::Condvar)>,
            observed_abort: Arc<AtomicBool>,
        }

        impl Job for GateStreamingJob {
            type Output = &'static str;

            fn run(self, context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
                emit("old-chunk-1");

                let (lock, cvar) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    if context.is_aborted() {
                        self.observed_abort.store(true, Ordering::SeqCst);
                        return;
                    }
                    open = cvar.wait(open).unwrap();
                }

                self.observed_abort
                    .store(context.is_aborted(), Ordering::SeqCst);
                if context.is_aborted() {
                    return;
                }

                emit("old-chunk-2");
            }
        }

        manager
            .submit(
                kind.clone(),
                JobPriority::Background,
                old_token,
                JobDelivery::Streaming,
                GateStreamingJob {
                    gate: Arc::clone(&gate),
                    observed_abort: Arc::clone(&observed_abort),
                },
            )
            .expect("old streaming job should submit");

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut accepted = Vec::new();
        while accepted.len() < 2 {
            let _ = manager.process_events(|event| accepted.push(event));
            assert!(
                Instant::now() < deadline,
                "timed out waiting for the first streaming chunk"
            );
            thread::sleep(Duration::from_millis(5));
        }

        manager
            .submit(
                kind.clone(),
                JobPriority::Background,
                new_token,
                JobDelivery::Streaming,
                TraceStreamingJob {
                    trace: Arc::new(Mutex::new(Vec::new())),
                },
            )
            .expect("new streaming job should submit");

        manager.abort_generation(kind.clone(), old_token);

        {
            let (lock, cvar) = &*gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cvar.notify_all();
        }

        let deadline = Instant::now() + Duration::from_secs(2);
        while !accepted
            .iter()
            .any(|event| matches!(event, JobEvent::Completed { token, .. } if *token == new_token))
        {
            let _ = manager.process_events(|event| accepted.push(event));
            assert!(
                Instant::now() < deadline,
                "timed out waiting for the superseding streaming job"
            );
            thread::sleep(Duration::from_millis(5));
        }

        assert!(observed_abort.load(Ordering::SeqCst));
        assert!(accepted.iter().any(|event| matches!(
            event,
            JobEvent::Started { token, .. } if *token == old_token
        )));
        assert!(accepted.iter().any(|event| matches!(
            event,
            JobEvent::Chunk { token, .. } if *token == old_token
        )));
        assert!(accepted.iter().any(|event| matches!(
            event,
            JobEvent::Started { token, .. } if *token == new_token
        )));
        assert!(accepted.iter().any(|event| matches!(
            event,
            JobEvent::Completed { token, .. } if *token == new_token
        )));
        assert!(!accepted.iter().any(|event| matches!(
            event,
            JobEvent::Completed { token, .. } if *token == old_token
        )));

        manager.shutdown();
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

            fn run(self, _context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
                let (lock, cvar) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cvar.wait(open).unwrap();
                }
                self.started.fetch_add(1, Ordering::SeqCst);
                self.trace.lock().unwrap().push(self.label);
                emit(self.label);
            }
        }

        handle
            .submit(
                JobKind::new("blocker"),
                JobPriority::Foreground,
                JobToken::new(1),
                JobDelivery::Once,
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
                JobDelivery::Once,
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
                JobDelivery::Once,
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
                .into_completed_payload()
                .expect("latest-only output should exist");
            match output {
                Some(JobPayload::Test(text)) => labels.push(text),
                other => panic!("expected text payload, got {:?}", other),
            }
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

            fn run(self, _context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
                let (lock, cvar) = &*self.gate;
                let mut open = lock.lock().unwrap();
                while !*open {
                    open = cvar.wait(open).unwrap();
                }
                self.trace.lock().unwrap().push(self.label);
                emit(self.label);
            }
        }

        manager
            .submit(
                JobKind::new("syntax"),
                JobPriority::Background,
                JobToken::new(1),
                JobDelivery::Once,
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
                JobDelivery::Once,
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
        while accepted.is_empty() {
            let _ = manager.process_events(|event| accepted.push(event));
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
            .into_completed_payload()
            .expect("accepted output should exist");
        assert_eq!(kind.as_str(), "syntax");
        assert_eq!(token.generation(), 2);
        match output {
            Some(JobPayload::Test(text)) => assert_eq!(text, "new"),
            other => panic!("expected text payload, got {:?}", other),
        }
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

            fn run(self, _context: &JobContext, _emit: &mut dyn FnMut(Self::Output)) {
                panic!("boom");
            }
        }

        manager
            .submit(
                JobKind::new("panic"),
                JobPriority::Foreground,
                JobToken::new(9),
                JobDelivery::Once,
                PanicJob,
            )
            .expect("panic job should submit");

        let mut failures = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while failures.is_empty() {
            let _ = manager.process_events(|event| failures.push(event));
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

            fn run(self, context: &JobContext, emit: &mut dyn FnMut(Self::Output)) {
                *self.observed.lock().unwrap() = Some((
                    context.kind().as_str().to_string(),
                    context.token(),
                    context.is_stopping(),
                ));
                emit(());
            }
        }

        handle
            .submit(
                JobKind::new("context"),
                JobPriority::Background,
                JobToken::new(7),
                JobDelivery::Once,
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
            JobDelivery::Once,
            TraceJob {
                label: "late",
                trace: Arc::new(Mutex::new(Vec::new())),
                started: Arc::new(AtomicUsize::new(0)),
            },
        );

        assert_eq!(result, Err(JobSubmitError::Stopped));
    }
}
