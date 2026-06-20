//! Generic background job scheduling primitives.

mod context;
mod error;
mod handle;
mod manager;
mod queue;
mod shared;
mod worker;

pub use context::JobContext;
pub use error::{JobError, JobSubmitError};
pub use handle::JobHandle;
pub use manager::JobManager;

use std::sync::mpsc::Sender;

/// Controls how queued jobs behave when newer work supersedes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobSubmissionMode {
    /// Keep every queued job until it runs or is superseded.
    Standard,
    /// Keep only the newest queued job for the same kind.
    LatestOnly,
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

/// A background job that can run on a worker thread and emit events.
pub trait BackgroundRunnable<K, E>: Send + 'static {
    /// Runs the job with shared cancellation context and an event sink.
    fn run(self, context: &JobContext<K>, event_tx: &Sender<E>);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Condvar, Mutex};
    use std::time::{Duration, Instant};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum TestKind {
        Blocking,
        Work,
        Abortable,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestEvent {
        Ran { kind: TestKind, generation: u64 },
        Current { generation: u64, is_current: bool },
        Aborted { generation: u64, is_aborted: bool },
    }

    enum TestJob {
        Emit,
        Block { gate: Arc<(Mutex<bool>, Condvar)> },
        ReportCurrent,
        ReportAborted,
    }

    impl BackgroundRunnable<TestKind, TestEvent> for TestJob {
        fn run(self, context: &JobContext<TestKind>, event_tx: &Sender<TestEvent>) {
            match self {
                Self::Emit => {
                    event_tx
                        .send(TestEvent::Ran {
                            kind: *context.kind(),
                            generation: context.token().generation(),
                        })
                        .ok();
                }
                Self::Block { gate } => {
                    let (lock, cvar) = &*gate;
                    let mut open = lock.lock().unwrap();
                    while !*open && !context.is_stopping() {
                        open = cvar.wait(open).unwrap();
                    }
                    event_tx
                        .send(TestEvent::Ran {
                            kind: *context.kind(),
                            generation: context.token().generation(),
                        })
                        .ok();
                }
                Self::ReportCurrent => {
                    event_tx
                        .send(TestEvent::Current {
                            generation: context.token().generation(),
                            is_current: context.is_current(),
                        })
                        .ok();
                }
                Self::ReportAborted => {
                    event_tx
                        .send(TestEvent::Aborted {
                            generation: context.token().generation(),
                            is_aborted: context.is_aborted(),
                        })
                        .ok();
                }
            }
        }
    }

    fn wait_for_event(handle: &JobHandle<TestKind, TestJob, TestEvent>) -> TestEvent {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(event) = handle.poll_event() {
                return event;
            }
            assert!(Instant::now() < deadline, "timed out waiting for job event");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn standard_submit_runs_job_and_emits_event() {
        let handle = JobHandle::new();

        handle
            .submit(TestKind::Work, JobToken::new(7), TestJob::Emit)
            .expect("job should submit");

        assert_eq!(
            wait_for_event(&handle),
            TestEvent::Ran {
                kind: TestKind::Work,
                generation: 7,
            }
        );
        handle.shutdown();
    }

    #[test]
    fn latest_only_discards_older_queued_jobs_for_same_kind() {
        let handle = JobHandle::new();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));

        handle
            .submit(
                TestKind::Blocking,
                JobToken::new(1),
                TestJob::Block {
                    gate: Arc::clone(&gate),
                },
            )
            .expect("blocking job should submit");
        std::thread::sleep(Duration::from_millis(25));

        handle
            .submit_latest_only(TestKind::Work, JobToken::new(1), TestJob::Emit)
            .expect("old job should submit");
        handle
            .submit_latest_only(TestKind::Work, JobToken::new(2), TestJob::Emit)
            .expect("new job should submit");

        {
            let (lock, cvar) = &*gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cvar.notify_all();
        }

        assert_eq!(
            wait_for_event(&handle),
            TestEvent::Ran {
                kind: TestKind::Blocking,
                generation: 1,
            }
        );
        assert_eq!(
            wait_for_event(&handle),
            TestEvent::Ran {
                kind: TestKind::Work,
                generation: 2,
            }
        );
        assert!(handle.poll_event().is_none());
        handle.shutdown();
    }

    #[test]
    fn context_reports_current_generation() {
        let handle = JobHandle::new();

        handle
            .submit_latest_only(TestKind::Work, JobToken::new(3), TestJob::ReportCurrent)
            .expect("job should submit");

        assert_eq!(
            wait_for_event(&handle),
            TestEvent::Current {
                generation: 3,
                is_current: true,
            }
        );
        assert!(handle.is_accepted(&TestKind::Work, JobToken::new(3)));
        assert!(!handle.is_accepted(&TestKind::Work, JobToken::new(2)));
        handle.shutdown();
    }

    #[test]
    fn context_reports_aborted_generation() {
        let handle = JobHandle::new();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));

        handle
            .submit(
                TestKind::Blocking,
                JobToken::new(1),
                TestJob::Block {
                    gate: Arc::clone(&gate),
                },
            )
            .expect("blocking job should submit");
        std::thread::sleep(Duration::from_millis(25));
        handle
            .submit(
                TestKind::Abortable,
                JobToken::new(4),
                TestJob::ReportAborted,
            )
            .expect("abortable job should submit");
        handle.abort_generation(TestKind::Abortable, JobToken::new(4));

        {
            let (lock, cvar) = &*gate;
            let mut open = lock.lock().unwrap();
            *open = true;
            cvar.notify_all();
        }

        assert_eq!(
            wait_for_event(&handle),
            TestEvent::Ran {
                kind: TestKind::Blocking,
                generation: 1,
            }
        );
        assert_eq!(
            wait_for_event(&handle),
            TestEvent::Aborted {
                generation: 4,
                is_aborted: true,
            }
        );
        handle.shutdown();
    }

    #[test]
    fn submit_after_shutdown_is_rejected() {
        let handle = JobHandle::new();

        handle.shutdown();

        let error = handle
            .submit(TestKind::Work, JobToken::new(1), TestJob::Emit)
            .expect_err("stopped worker should reject submissions");
        assert_eq!(error, JobSubmitError::Stopped);
    }

    #[test]
    fn manager_tracks_redraw_flag() {
        let manager: JobManager<TestKind, TestJob, TestEvent> = JobManager::with_workers(1);

        assert!(!manager.redraw_requested());
        manager.mark_redraw_requested();
        assert!(manager.redraw_requested());
        assert!(manager.take_redraw_requested());
        assert!(!manager.redraw_requested());
        manager.shutdown();
    }
}
