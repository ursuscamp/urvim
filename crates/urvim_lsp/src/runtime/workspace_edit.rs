//! Workspace edit → `LspRuntimeEffect` conversion.
//!
//! Instead of applying workspace edits directly to editor state (which would
//! require access to core globals), this module converts an LSP `WorkspaceEdit`
//! into a list of `LspRuntimeEffect` values. Core drains these effects and
//! applies them to buffers, the filesystem, and the buffer pool.

use std::collections::BTreeMap;
use std::path::PathBuf;

use lsp_types::{CreateFile, DeleteFile, DocumentChangeOperation, RenameFile, ResourceOp};

use crate::document::{LspRuntimeEffect, LspTextEdit, LspWorkspaceFileOperation};
use crate::position::text_range_from_lsp;

use super::session::uri_to_file_path;

/// Converts an LSP `WorkspaceEdit` into a list of `LspRuntimeEffect` values.
///
/// Text edits become `ApplyTextEdits` effects (one per URI). Resource
/// operations become `WorkspaceFileOperation` effects. Core is responsible
/// for applying the effects to editor state.
pub fn workspace_edit_to_effects(
    edit: &lsp_types::WorkspaceEdit,
) -> Result<Vec<LspRuntimeEffect>, String> {
    let mut effects = Vec::new();

    if let Some(changes) = edit.changes.as_ref() {
        for (uri, edits) in changes {
            let path = uri_to_file_path(&uri.to_string())?;
            let lsp_edits = edits
                .iter()
                .map(|edit| LspTextEdit {
                    range: text_range_from_lsp(edit.range),
                    text: edit.new_text.clone(),
                })
                .collect::<Vec<_>>();
            effects.push(LspRuntimeEffect::ApplyTextEdits {
                path,
                edits: lsp_edits,
            });
        }
    }

    if let Some(changes) = edit.document_changes.as_ref() {
        match changes {
            lsp_types::DocumentChanges::Edits(edits) => {
                let mut grouped = BTreeMap::<String, Vec<lsp_types::TextEdit>>::new();
                for text_document_edit in edits {
                    let uri = text_document_edit.text_document.uri.to_string();
                    let edits = text_document_edit
                        .edits
                        .iter()
                        .map(|edit| match edit {
                            lsp_types::OneOf::Left(text_edit) => Ok(text_edit.clone()),
                            lsp_types::OneOf::Right(annotated) => Ok(annotated.text_edit.clone()),
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    grouped.entry(uri).or_default().extend(edits);
                }

                for (uri, edits) in grouped {
                    let path = uri_to_file_path(&uri)?;
                    let lsp_edits = edits
                        .iter()
                        .map(|edit| LspTextEdit {
                            range: text_range_from_lsp(edit.range),
                            text: edit.new_text.clone(),
                        })
                        .collect::<Vec<_>>();
                    effects.push(LspRuntimeEffect::ApplyTextEdits {
                        path,
                        edits: lsp_edits,
                    });
                }
            }
            lsp_types::DocumentChanges::Operations(operations) => {
                for operation in operations {
                    match operation {
                        DocumentChangeOperation::Edit(text_document_edit) => {
                            let uri = text_document_edit.text_document.uri.clone();
                            let edits = text_document_edit
                                .edits
                                .iter()
                                .map(|edit| match edit {
                                    lsp_types::OneOf::Left(text_edit) => Ok(text_edit.clone()),
                                    lsp_types::OneOf::Right(annotated) => {
                                        Ok(annotated.text_edit.clone())
                                    }
                                })
                                .collect::<Result<Vec<_>, String>>()?;
                            let path = uri_to_file_path(&uri.to_string())?;
                            let lsp_edits = edits
                                .iter()
                                .map(|edit| LspTextEdit {
                                    range: text_range_from_lsp(edit.range),
                                    text: edit.new_text.clone(),
                                })
                                .collect::<Vec<_>>();
                            effects.push(LspRuntimeEffect::ApplyTextEdits {
                                path,
                                edits: lsp_edits,
                            });
                        }
                        DocumentChangeOperation::Op(resource_op) => match resource_op {
                            ResourceOp::Create(create) => {
                                let effect = create_file_effect(create)?;
                                effects.push(effect);
                            }
                            ResourceOp::Rename(rename) => {
                                let effect = rename_file_effect(rename)?;
                                effects.push(effect);
                            }
                            ResourceOp::Delete(delete) => {
                                let effect = delete_file_effect(delete)?;
                                effects.push(effect);
                            }
                        },
                    }
                }
            }
        }
    }

    Ok(effects)
}

fn create_file_effect(create: &CreateFile) -> Result<LspRuntimeEffect, String> {
    let path = uri_to_file_path(&create.uri.to_string())?;
    let options = create.options.as_ref();
    let overwrite = options
        .and_then(|options| options.overwrite)
        .unwrap_or(false);
    let ignore_if_exists = options
        .and_then(|options| options.ignore_if_exists)
        .unwrap_or(false);
    Ok(LspRuntimeEffect::WorkspaceFileOperation {
        operation: LspWorkspaceFileOperation::Create {
            path,
            overwrite,
            ignore_if_exists,
        },
    })
}

fn rename_file_effect(rename: &RenameFile) -> Result<LspRuntimeEffect, String> {
    let old_path = uri_to_file_path(&rename.old_uri.to_string())?;
    let new_path = uri_to_file_path(&rename.new_uri.to_string())?;
    let options = rename.options.as_ref();
    let overwrite = options
        .and_then(|options| options.overwrite)
        .unwrap_or(false);
    let ignore_if_exists = options
        .and_then(|options| options.ignore_if_exists)
        .unwrap_or(false);
    Ok(LspRuntimeEffect::WorkspaceFileOperation {
        operation: LspWorkspaceFileOperation::Rename {
            old_path,
            new_path,
            overwrite,
            ignore_if_exists,
        },
    })
}

fn delete_file_effect(delete: &DeleteFile) -> Result<LspRuntimeEffect, String> {
    let path = uri_to_file_path(&delete.uri.to_string())?;
    let options = delete.options.as_ref();
    let ignore_if_not_exists = options
        .and_then(|options| options.ignore_if_not_exists)
        .unwrap_or(false);
    Ok(LspRuntimeEffect::WorkspaceFileOperation {
        operation: LspWorkspaceFileOperation::Delete {
            path,
            ignore_if_not_exists,
        },
    })
}

/// Holds the resolved file paths from a `WorkspaceFileOperation` for core
/// to use when notifying the LSP runtime of the rename/delete.
#[derive(Debug, Clone)]
pub enum ResolvedFileOperation {
    Create {
        path: PathBuf,
    },
    Rename {
        old_path: PathBuf,
        new_path: PathBuf,
    },
    Delete {
        path: PathBuf,
    },
}

/// Extracts the resolved file paths from a `LspWorkspaceFileOperation`.
pub fn resolve_file_operation(operation: &LspWorkspaceFileOperation) -> ResolvedFileOperation {
    match operation {
        LspWorkspaceFileOperation::Create { path, .. } => {
            ResolvedFileOperation::Create { path: path.clone() }
        }
        LspWorkspaceFileOperation::Rename {
            old_path, new_path, ..
        } => ResolvedFileOperation::Rename {
            old_path: old_path.clone(),
            new_path: new_path.clone(),
        },
        LspWorkspaceFileOperation::Delete { path, .. } => {
            ResolvedFileOperation::Delete { path: path.clone() }
        }
    }
}
