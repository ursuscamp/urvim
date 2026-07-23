//! Plugin-owned grouping for buffer undo history.

use std::cell::RefCell;
use std::collections::HashMap;

use urvim_core::buffer::{BufferId, UndoCheckpoint};
use urvim_core::globals;

thread_local! {
    static TRANSACTIONS: RefCell<HashMap<(String, BufferId), UndoCheckpoint>> =
        RefCell::new(HashMap::new());
}

/// Starts an undo-history transaction owned by `plugin`.
pub(in crate::plugin) fn begin(plugin: &str, buffer_id: BufferId) -> Result<(), String> {
    let key = (plugin.to_string(), buffer_id);
    if TRANSACTIONS.with(|transactions| transactions.borrow().contains_key(&key)) {
        return Err(format!(
            "buffer transaction already active for buffer_id {}",
            buffer_id.get()
        ));
    }

    let checkpoint = globals::with_buffer(buffer_id, |buffer| buffer.undo_checkpoint())
        .ok_or_else(|| format!("unknown buffer_id {}", buffer_id.get()))?;
    TRANSACTIONS.with(|transactions| {
        transactions.borrow_mut().insert(key, checkpoint);
    });
    Ok(())
}

/// Finishes one owned transaction and combines its undo snapshots.
pub(in crate::plugin) fn end(plugin: &str, buffer_id: BufferId) -> Result<(), String> {
    let key = (plugin.to_string(), buffer_id);
    let checkpoint = TRANSACTIONS
        .with(|transactions| transactions.borrow_mut().remove(&key))
        .ok_or_else(|| {
            format!(
                "no buffer transaction active for buffer_id {}",
                buffer_id.get()
            )
        })?;
    commit(buffer_id, checkpoint)
}

/// Returns whether `plugin` has an open transaction for `buffer_id`.
pub(in crate::plugin) fn is_active(plugin: &str, buffer_id: BufferId) -> bool {
    TRANSACTIONS.with(|transactions| {
        transactions
            .borrow()
            .contains_key(&(plugin.to_string(), buffer_id))
    })
}

/// Finalizes every transaction left open by a completed plugin callback.
pub(in crate::plugin) fn finish_plugin(plugin: &str) {
    let pending = TRANSACTIONS.with(|transactions| {
        let mut transactions = transactions.borrow_mut();
        let keys = transactions
            .keys()
            .filter(|(owner, _)| owner == plugin)
            .cloned()
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| {
                transactions
                    .remove(&key)
                    .map(|checkpoint| (key.1, checkpoint))
            })
            .collect::<Vec<_>>()
    });

    for (buffer_id, checkpoint) in pending {
        if let Err(error) = commit(buffer_id, checkpoint) {
            tracing::warn!(
                plugin,
                ?buffer_id,
                error,
                "failed to finalize plugin buffer transaction"
            );
        }
    }
}

fn commit(buffer_id: BufferId, checkpoint: UndoCheckpoint) -> Result<(), String> {
    let committed =
        globals::with_buffer_mut(buffer_id, |buffer| buffer.squash_undo_history(checkpoint))
            .ok_or_else(|| format!("unknown buffer_id {}", buffer_id.get()))?;
    if committed {
        Ok(())
    } else {
        Err(format!(
            "buffer undo history changed during transaction for buffer_id {}",
            buffer_id.get()
        ))
    }
}
