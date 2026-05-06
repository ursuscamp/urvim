use super::Layout;
use crate::background::{JobEvent, JobKind, JobPayload};

impl Layout {
    /// Routes LSP-related background job events.
    pub fn dispatch_lsp_job_event(&mut self, event: JobEvent) {
        match event {
            JobEvent::Started { .. } => {}
            JobEvent::Completed {
                kind,
                payload: Some(JobPayload::LspRename(result)),
                ..
            } if matches!(kind, JobKind::LspRename(_)) => {
                if let Err(error) = result {
                    crate::notify_error!("LSP rename failed: {}", error);
                } else {
                    crate::globals::request_notification_redraw();
                }
            }
            JobEvent::Completed { kind, .. } if matches!(kind, JobKind::LspRename(_)) => {}
            JobEvent::Failed { kind, .. } if matches!(kind, JobKind::LspRename(_)) => {
                crate::notify_error!("LSP rename failed: job worker reported failure");
            }
            _ => {}
        }
    }
}
