use crate::buffer::BufferId;

use super::{JobEvent, JobKind, JobToken};

#[test]
fn job_event_helpers_report_kind_and_token() {
    let kind = JobKind::BufferCacheRefresh(BufferId::new(7));
    let token = JobToken::new(7);
    let event = JobEvent::Started {
        kind: kind.clone(),
        token,
    };

    assert_eq!(event.kind(), &kind);
    assert_eq!(event.token(), token);
    assert!(event.is_started());
    assert!(!event.is_terminal());
}
