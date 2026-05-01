use std::sync::Arc;

use super::context::JobContext;
use super::shared::JobShared;

/// Worker thread main loop.
pub fn worker_loop(shared: Arc<JobShared>) {
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
        let context = JobContext::new(
            kind,
            token,
            Arc::clone(&shared.stopping),
            Arc::clone(&shared.latest_generations),
            Arc::clone(&shared.aborted_generations),
        );
        job.job.run(&context, &shared.event_tx);
    }
}
