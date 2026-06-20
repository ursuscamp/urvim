use url::Url;

use crate::buffer::{Buffer, BufferId};
use urvim_lsp::document::LspDocumentSnapshot;

/// Builds an `LspDocumentSnapshot` from core buffer state.
///
/// Returns `None` when the buffer has no file path.
pub fn snapshot_for_buffer(
    buffer: &Buffer,
    buffer_id: BufferId,
    version: i32,
) -> Option<LspDocumentSnapshot> {
    let path = buffer.path()?;
    let uri = Url::from_file_path(path.as_path()).ok()?.to_string();

    Some(LspDocumentSnapshot {
        id: buffer_id,
        uri,
        path: path.as_path().to_path_buf(),
        language_id: buffer.syntax_name().to_string(),
        version,
        generation: buffer.syntax_generation(),
        text: buffer.text_snapshot(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Buffer;
    use crate::path::AbsolutePath;
    use std::fs;
    use std::path::PathBuf;
    use urvim_text::{TextRef, TextSnapshot};

    fn test_buffer_with_path(path: &str) -> (Buffer, PathBuf) {
        let p = std::env::temp_dir().join(path);
        let _ = fs::write(&p, "hello\nworld\n");
        let abs = AbsolutePath::new(p.clone()).expect("abs path");
        let mut buffer = Buffer::from_str("hello\nworld\n");
        buffer.set_path(abs);
        (buffer, p)
    }

    #[test]
    fn snapshot_for_buffer_returns_some_for_file_buffer() {
        let (buffer, p) = test_buffer_with_path("lsp_snapshot_test_1.rs");
        let buffer_id = BufferId::new(42);
        let snapshot = snapshot_for_buffer(&buffer, buffer_id, 1).expect("snapshot");
        assert_eq!(snapshot.id, buffer_id);
        assert!(snapshot.uri.contains("lsp_snapshot_test_1.rs"));
        assert_eq!(snapshot.path, p);
        assert_eq!(snapshot.version, 1);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn snapshot_for_buffer_uses_syntax_name() {
        let (buffer, p) = test_buffer_with_path("lsp_snapshot_test_2.rs");
        let snapshot = snapshot_for_buffer(&buffer, BufferId::new(1), 1).expect("snapshot");
        assert_eq!(snapshot.language_id, "rust");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn snapshot_for_buffer_captures_text() {
        let (buffer, p) = test_buffer_with_path("lsp_snapshot_test_3.rs");
        let snapshot = snapshot_for_buffer(&buffer, BufferId::new(1), 1).expect("snapshot");
        assert_eq!(snapshot.text.text().to_text(), "hello\nworld");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn snapshot_for_buffer_generation_is_syntax_generation() {
        let (buffer, p) = test_buffer_with_path("lsp_snapshot_test_4.rs");
        let generation = buffer.syntax_generation();
        let snapshot = snapshot_for_buffer(&buffer, BufferId::new(1), 1).expect("snapshot");
        assert_eq!(snapshot.generation, generation);
        let _ = fs::remove_file(&p);
    }
}
