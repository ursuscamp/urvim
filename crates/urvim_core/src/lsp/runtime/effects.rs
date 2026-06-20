//! LspRuntimeEffect application.
//!
//! Applies effects from `urvim_lsp` to editor state: buffer mutation,
//! diagnostics storage, filesystem operations, and UI notifications.

use std::fs::{self, OpenOptions};
use std::path::Path;

use lsp_types::PositionEncodingKind;

use urvim_lsp::document::{LspRuntimeEffect, LspTextEdit, LspWorkspaceFileOperation};
use urvim_lsp::position::text_encoding_from_lsp;
use urvim_text::{PieceTable, TextRef, TextSnapshot};

use crate::globals;

use super::LspRuntime;
use super::position_to_cursor;

impl LspRuntime {
    pub(super) fn drain_effects(&mut self) {
        let effects = self.runtime.drain_effects();
        self.apply_effects(effects);
    }

    pub(super) fn apply_effects(&mut self, effects: Vec<LspRuntimeEffect>) {
        for effect in effects {
            self.apply_lsp_effect(effect);
        }
    }

    fn apply_lsp_effect(&mut self, effect: LspRuntimeEffect) {
        match effect {
            LspRuntimeEffect::Diagnostics {
                buffer_id,
                server_name,
                diagnostics,
            } => {
                let encoding = self.runtime.position_encoding_for_buffer(buffer_id);
                if let Some(lines) = globals::with_buffer(buffer_id, |b| b.text_snapshot()) {
                    let converted = diagnostics
                        .into_iter()
                        .filter_map(|d| convert_diagnostic(&lines, d, encoding.clone()))
                        .collect();
                    globals::with_diagnostics_store(|store| {
                        store.set(buffer_id, &server_name, converted)
                    });
                }
                globals::request_inlay_hint_retry();
                globals::request_notification_redraw();
            }
            LspRuntimeEffect::ClearDiagnostics {
                buffer_id,
                server_name,
            } => {
                globals::with_diagnostics_store(|store| store.clear(buffer_id, &server_name));
            }
            LspRuntimeEffect::OpenDocument { path } => {
                if let Err(error) = globals::open_buffer(&path) {
                    tracing::warn!(?error, path = ?path, "failed to open buffer for LSP effect");
                }
            }
            LspRuntimeEffect::ApplyTextEdits { path, edits } => {
                if let Err(error) = self.apply_text_edits_to_buffer(&path, &edits) {
                    tracing::warn!(?error, path = ?path, "failed to apply LSP text edits");
                }
            }
            LspRuntimeEffect::WorkspaceFileOperation { operation } => {
                if let Err(error) = self.apply_workspace_file_operation(operation) {
                    tracing::warn!(?error, "failed to apply LSP workspace file operation");
                }
            }
            LspRuntimeEffect::RequestRedraw => {
                globals::request_notification_redraw();
            }
            LspRuntimeEffect::RequestInlayHintRetry => {
                globals::request_inlay_hint_retry();
            }
        }
    }

    fn apply_text_edits_to_buffer(
        &mut self,
        path: &Path,
        edits: &[LspTextEdit],
    ) -> Result<(), String> {
        let buffer_id = globals::open_buffer(path).map_err(|e| e.to_string())?;
        let encoding = self.runtime.position_encoding_for_buffer(buffer_id);
        let text_encoding = text_encoding_from_lsp(encoding);

        let mut sorted_edits = edits.to_vec();
        sorted_edits.sort_by(|left, right| {
            right
                .range
                .start
                .line
                .cmp(&left.range.start.line)
                .then_with(|| right.range.start.character.cmp(&left.range.start.character))
        });

        let cursor_edits = globals::with_buffer(buffer_id, |buffer| {
            sorted_edits
                .iter()
                .map(|edit| {
                    let start = buffer.cursor_for_position(edit.range.start, text_encoding)?;
                    let end = buffer.cursor_for_position(edit.range.end, text_encoding)?;
                    Some((start, end, edit.text.clone()))
                })
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| "failed to read buffer for workspace edit".to_string())?
        .ok_or_else(|| "failed to convert workspace edit positions".to_string())?;

        let applied =
            globals::with_buffer_mut(buffer_id, |buffer| buffer.apply_text_edits(&cursor_edits))
                .unwrap_or(false);

        if !applied {
            return Err("failed to apply workspace edit".to_string());
        }

        globals::with_buffer_pool(|pool| pool.request_buffer_cache_refresh(buffer_id));
        crate::session::mark_dirty();
        globals::request_notification_redraw();
        Ok(())
    }

    fn apply_workspace_file_operation(
        &mut self,
        operation: LspWorkspaceFileOperation,
    ) -> Result<(), String> {
        match operation {
            LspWorkspaceFileOperation::Create {
                path,
                overwrite,
                ignore_if_exists,
            } => self.apply_create_file(&path, overwrite, ignore_if_exists),
            LspWorkspaceFileOperation::Rename {
                old_path,
                new_path,
                overwrite,
                ignore_if_exists,
            } => self.apply_rename_file(&old_path, &new_path, overwrite, ignore_if_exists),
            LspWorkspaceFileOperation::Delete {
                path,
                ignore_if_not_exists,
            } => self.apply_delete_file(&path, ignore_if_not_exists),
        }
    }

    fn apply_create_file(
        &mut self,
        path: &Path,
        overwrite: bool,
        ignore_if_exists: bool,
    ) -> Result<(), String> {
        let exists = path.exists();
        if exists {
            if ignore_if_exists {
                return Ok(());
            }
            if !overwrite {
                return Err(format!("file already exists: {}", path.display()));
            }
            let buffer_id = globals::with_buffer_pool(|pool| {
                crate::AbsolutePath::from_path(path).and_then(|abs| pool.buffer_id_for_path(&abs))
            });
            remove_path(path)?;
            if let Some(buffer_id) = buffer_id {
                globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id));
                globals::enqueue_workspace_file_operation_notification(
                    globals::WorkspaceFileOperationNotification::Delete {
                        path: path.to_path_buf(),
                        buffer_id: Some(buffer_id),
                    },
                );
            }
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .map_err(|e| e.to_string())?;

        globals::enqueue_workspace_file_operation_notification(
            globals::WorkspaceFileOperationNotification::Create {
                path: path.to_path_buf(),
            },
        );
        crate::session::mark_dirty();
        globals::request_notification_redraw();
        Ok(())
    }

    fn apply_rename_file(
        &mut self,
        old_path: &Path,
        new_path: &Path,
        overwrite: bool,
        ignore_if_exists: bool,
    ) -> Result<(), String> {
        if !old_path.exists() {
            return Err(format!("file does not exist: {}", old_path.display()));
        }

        let target_exists = new_path.exists();
        if target_exists {
            if ignore_if_exists {
                return Ok(());
            }
            if !overwrite {
                return Err(format!("file already exists: {}", new_path.display()));
            }
            remove_path(new_path)?;
        }

        if let Some(parent) = new_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        let replaced_buffer_id = globals::with_buffer_pool(|pool| {
            crate::AbsolutePath::from_path(new_path).and_then(|abs| pool.buffer_id_for_path(&abs))
        });
        fs::rename(old_path, new_path).map_err(|e| e.to_string())?;

        if let Some(buffer_id) = replaced_buffer_id {
            globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id));
            globals::enqueue_workspace_file_operation_notification(
                globals::WorkspaceFileOperationNotification::Delete {
                    path: new_path.to_path_buf(),
                    buffer_id: Some(buffer_id),
                },
            );
        }

        let source_buffer_id = globals::with_buffer_pool(|pool| {
            crate::AbsolutePath::from_path(old_path).and_then(|abs| pool.buffer_id_for_path(&abs))
        });

        if let Some(source_buffer_id) = source_buffer_id {
            globals::with_buffer_pool(|pool| pool.rename_buffer_path(source_buffer_id, new_path))
                .map_err(|e| e.to_string())?;

            let text =
                globals::with_buffer(source_buffer_id, |b| b.text_snapshot().text().to_text());
            if let Some(text) = text {
                self.runtime.handle_file_renamed(old_path, new_path, &text);
            }
        }

        globals::enqueue_workspace_file_operation_notification(
            globals::WorkspaceFileOperationNotification::Rename {
                old_path: old_path.to_path_buf(),
                new_path: new_path.to_path_buf(),
            },
        );
        crate::session::mark_dirty();
        globals::request_notification_redraw();
        Ok(())
    }

    fn apply_delete_file(&mut self, path: &Path, ignore_if_not_exists: bool) -> Result<(), String> {
        let exists = path.exists();
        if !exists {
            if ignore_if_not_exists {
                return Ok(());
            }
            return Err(format!("file does not exist: {}", path.display()));
        }

        let buffer_id = globals::with_buffer_pool(|pool| {
            crate::AbsolutePath::from_path(path).and_then(|abs| pool.buffer_id_for_path(&abs))
        });
        remove_path(path)?;

        if let Some(buffer_id) = buffer_id {
            globals::with_buffer_pool(|pool| pool.remove_buffer(buffer_id));
            globals::enqueue_workspace_file_operation_notification(
                globals::WorkspaceFileOperationNotification::Delete {
                    path: path.to_path_buf(),
                    buffer_id: Some(buffer_id),
                },
            );
            self.runtime.handle_file_deleted(buffer_id);
        }

        crate::session::mark_dirty();
        globals::request_notification_redraw();
        Ok(())
    }
}

fn remove_path(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())
    }
}

fn convert_diagnostic(
    lines: &PieceTable,
    diagnostic: lsp_types::Diagnostic,
    encoding: PositionEncodingKind,
) -> Option<lsp_types::Diagnostic> {
    let start = position_to_cursor(lines, diagnostic.range.start, encoding.clone())?;
    let end = position_to_cursor(lines, diagnostic.range.end, encoding)?;
    let mut diagnostic = diagnostic;
    diagnostic.range = lsp_types::Range::new(
        lsp_types::Position::new(start.line as u32, start.col as u32),
        lsp_types::Position::new(end.line as u32, end.col as u32),
    );
    Some(diagnostic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;
    use urvim_lsp::runtime::workspace_edit::workspace_edit_to_effects;

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "urvim-lsp-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn empty_runtime() -> LspRuntime {
        LspRuntime::new(&Config::default())
    }

    #[test]
    fn workspace_edit_applies_text_changes_via_effects() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

        let temp = temp_dir("edit-effects");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("sample.rs");
        std::fs::write(&path, "hello world").expect("write");
        let uri: lsp_types::Uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: Some(std::collections::HashMap::from([(
                uri,
                vec![lsp_types::TextEdit {
                    range: lsp_types::Range {
                        start: lsp_types::Position::new(0, 6),
                        end: lsp_types::Position::new(0, 11),
                    },
                    new_text: "urvim".to_string(),
                }],
            )])),
            document_changes: None,
            change_annotations: None,
        };

        let effects = workspace_edit_to_effects(&edit).expect("convert to effects");
        let mut runtime = empty_runtime();
        runtime.apply_effects(effects);

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let text =
            crate::globals::with_buffer(buffer_id, |b| b.as_str()).expect("buffer should exist");
        assert_eq!(text, "hello urvim");
    }

    #[test]
    fn workspace_edit_applies_multiple_edits_in_one_file_via_effects() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());

        let temp = temp_dir("edit-many-effects");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("sample.rs");
        std::fs::write(&path, "abcdef").expect("write");
        let uri: lsp_types::Uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: Some(std::collections::HashMap::from([(
                uri,
                vec![
                    lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(0, 1),
                            end: lsp_types::Position::new(0, 2),
                        },
                        new_text: "X".to_string(),
                    },
                    lsp_types::TextEdit {
                        range: lsp_types::Range {
                            start: lsp_types::Position::new(0, 4),
                            end: lsp_types::Position::new(0, 5),
                        },
                        new_text: "Y".to_string(),
                    },
                ],
            )])),
            document_changes: None,
            change_annotations: None,
        };

        let effects = workspace_edit_to_effects(&edit).expect("convert to effects");
        let mut runtime = empty_runtime();
        runtime.apply_effects(effects);

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let text =
            crate::globals::with_buffer(buffer_id, |b| b.as_str()).expect("buffer should exist");
        assert_eq!(text, "aXcdYf");
    }

    #[test]
    fn workspace_edit_resource_create_via_effects() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
        crate::globals::clear_workspace_file_operation_notifications();

        let temp = temp_dir("create-effects");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("created.rs");
        let uri: lsp_types::Uri = url::Url::from_file_path(&path)
            .expect("uri")
            .to_string()
            .parse()
            .expect("uri");

        let edit = lsp_types::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Create(
                    lsp_types::CreateFile {
                        uri,
                        options: None,
                        annotation_id: None,
                    },
                )),
            ])),
            change_annotations: None,
        };

        let effects = workspace_edit_to_effects(&edit).expect("convert to effects");
        assert_eq!(effects.len(), 1);
        let mut runtime = empty_runtime();
        runtime.apply_effects(effects);

        assert!(path.exists());
        assert!(crate::globals::take_workspace_file_operation_notification().is_some());
    }

    #[test]
    fn workspace_edit_renames_loaded_buffer_via_effects() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
        crate::globals::clear_workspace_file_operation_notifications();

        let temp = temp_dir("rename-effects");
        std::fs::create_dir_all(&temp).expect("root");
        let old_path = temp.join("old.rs");
        let new_path = temp.join("new.rs");
        std::fs::write(&old_path, "hello world").expect("write");

        let buffer_id = crate::globals::open_buffer(&old_path).expect("buffer should open");
        let edit = lsp_types::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Rename(
                    lsp_types::RenameFile {
                        old_uri: url::Url::from_file_path(&old_path)
                            .expect("old uri")
                            .to_string()
                            .parse()
                            .expect("uri"),
                        new_uri: url::Url::from_file_path(&new_path)
                            .expect("new uri")
                            .to_string()
                            .parse()
                            .expect("uri"),
                        options: None,
                        annotation_id: None,
                    },
                )),
            ])),
            change_annotations: None,
        };

        let effects = workspace_edit_to_effects(&edit).expect("convert to effects");
        let mut runtime = empty_runtime();
        runtime.apply_effects(effects);

        assert!(new_path.exists());
        assert!(!old_path.exists());
        assert_eq!(
            crate::globals::with_buffer_pool(|pool| {
                pool.buffer_id_for_path(&crate::AbsolutePath::from_path(&new_path).expect("abs"))
            }),
            Some(buffer_id)
        );
    }

    #[test]
    fn workspace_edit_deletes_loaded_buffer_via_effects() {
        let _lock = crate::globals::buffer_pool_test_lock();
        crate::globals::with_buffer_pool(|pool| *pool = crate::buffer::BufferPool::new());
        crate::globals::clear_workspace_file_operation_notifications();

        let temp = temp_dir("delete-effects");
        std::fs::create_dir_all(&temp).expect("root");
        let path = temp.join("delete.rs");
        std::fs::write(&path, "hello world").expect("write");

        let buffer_id = crate::globals::open_buffer(&path).expect("buffer should open");
        let edit = lsp_types::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp_types::DocumentChanges::Operations(vec![
                lsp_types::DocumentChangeOperation::Op(lsp_types::ResourceOp::Delete(
                    lsp_types::DeleteFile {
                        uri: url::Url::from_file_path(&path)
                            .expect("uri")
                            .to_string()
                            .parse()
                            .expect("uri"),
                        options: None,
                    },
                )),
            ])),
            change_annotations: None,
        };

        let effects = workspace_edit_to_effects(&edit).expect("convert to effects");
        let mut runtime = empty_runtime();
        runtime.apply_effects(effects);

        assert!(!path.exists());
        assert!(crate::globals::with_buffer(buffer_id, |_| ()).is_none());
        assert!(matches!(
            crate::globals::take_workspace_file_operation_notification(),
            Some(crate::globals::WorkspaceFileOperationNotification::Delete { .. })
        ));
    }
}
