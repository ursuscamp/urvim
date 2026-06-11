use super::Layout;
use crate::ui::picker::git::{GitPickerAction, GitPickerSource, GitPickerWidget};
use crate::ui::{UiEvent, UiEventResult};
use crate::widget::Widget;
use std::ffi::OsStr;

impl Layout {
    /// Opens the git picker overlay.
    pub(in crate::layout) fn open_git_picker(&mut self) {
        self.close_all_dialogs();

        match std::env::current_dir() {
            Ok(cwd) => {
                let mut picker =
                    GitPickerWidget::new(GitPickerSource::with_jobs(cwd, self.jobs.clone()));
                picker.set_label("Git");
                self.dialogs.git_picker = Some(picker);
                self.refresh_git_picker_prompt();
                if let Some(picker) = self.dialogs.git_picker.as_mut() {
                    picker.restart_search();
                }
            }
            Err(error) => {
                crate::notify_error!("Failed to open git picker: {}", error);
            }
        }
    }

    /// Closes the git picker overlay.
    pub(in crate::layout) fn close_git_picker(&mut self) {
        if let Some(picker) = self.dialogs.git_picker.as_mut() {
            picker.close();
        }
        self.dialogs.git_picker = None;
    }

    /// Returns true when the git picker is open.
    pub(in crate::layout) fn git_picker_is_open(&self) -> bool {
        self.dialogs
            .git_picker
            .as_ref()
            .is_some_and(GitPickerWidget::is_open)
    }

    /// Returns a mutable reference to the git picker when open.
    pub(in crate::layout) fn git_picker_mut(&mut self) -> Option<&mut GitPickerWidget> {
        self.dialogs.git_picker.as_mut()
    }

    /// Routes an event to the git picker overlay.
    pub(in crate::layout) fn handle_git_picker_event(&mut self, event: &UiEvent) -> UiEventResult {
        let Some(picker) = self.dialogs.git_picker.as_mut() else {
            return UiEventResult::NotHandled;
        };

        let mut ctx = crate::ui::UiContext;
        let result = picker.handle_ui_event(event, &mut ctx);
        if result.handled() && !picker.is_open() {
            self.close_git_picker();
        }

        result
    }

    fn refresh_git_picker_prompt(&mut self) {
        if let Some(picker) = self.dialogs.git_picker.as_mut() {
            let mode = picker.source_mut().query_mode();
            picker.set_query_prompt_segments(GitPickerSource::query_prompt_segments(mode));
        }
    }

    pub(in crate::layout) fn open_git_picker_discard_confirmation(
        &mut self,
        action: &GitPickerAction,
    ) {
        let label = action
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| action.path.to_string_lossy().into_owned());
        self.open_confirmation_box(
            format!("Discard changes in {label}?"),
            crate::ui::Command::GitPickerDiscardConfirmed(action.clone()),
        );
    }

    pub(in crate::layout) fn execute_git_picker_toggle_stage(
        &mut self,
        action: &GitPickerAction,
    ) -> bool {
        let Some(name) = action.path.file_name() else {
            crate::notify_error!("Git stage failed: missing file name");
            return true;
        };

        let result = if action.untracked || !action.staged {
            run_git_command(
                action.path.as_path(),
                [OsStr::new("add"), OsStr::new("--"), name],
            )
        } else {
            run_git_command(
                action.path.as_path(),
                [
                    OsStr::new("restore"),
                    OsStr::new("--staged"),
                    OsStr::new("--"),
                    name,
                ],
            )
        };

        if let Err(error) = result {
            crate::notify_error!("Git stage failed: {}", error);
        }

        self.refresh_git_picker_after_action();
        true
    }

    pub(in crate::layout) fn execute_git_picker_discard(
        &mut self,
        action: &GitPickerAction,
    ) -> bool {
        let Some(name) = action.path.file_name() else {
            crate::notify_error!("Git discard failed: missing file name");
            return true;
        };

        let result = if action.untracked {
            std::fs::remove_file(&action.path).map_err(|error| error.to_string())
        } else {
            run_git_command(
                action.path.as_path(),
                [
                    OsStr::new("restore"),
                    OsStr::new("--source=HEAD"),
                    OsStr::new("--staged"),
                    OsStr::new("--worktree"),
                    OsStr::new("--"),
                    name,
                ],
            )
        };

        if let Err(error) = result {
            crate::notify_error!("Git discard failed: {}", error);
        }

        self.refresh_git_picker_after_action();
        true
    }

    fn refresh_git_picker_after_action(&mut self) {
        if let Some(picker) = self.dialogs.git_picker.as_mut() {
            picker.restart_search();
        }
    }
}

fn run_git_command<const N: usize>(
    path: &std::path::Path,
    args: [&OsStr; N],
) -> Result<(), String> {
    let Some(parent) = path.parent() else {
        return Err("missing parent directory".to_string());
    };

    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(parent)
        .args(args)
        .status()
        .map_err(|error| error.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("git command failed with status {}", status))
    }
}
