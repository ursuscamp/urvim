use super::command_line::CommandLineState;
use crate::ui::completion::CompletionWidget;
use crate::ui::confirmation_box::ConfirmationBox;
use crate::ui::diagnostic_hover::DiagnosticHoverWidget;
use crate::ui::hover::HoverWidget;
use crate::ui::lsp_rename::LspRenamePrompt;
use crate::ui::picker::code_actions::CodeActionsPickerWidget;
use crate::ui::picker::colorscheme::ColorschemePickerWidget;
use crate::ui::picker::doc_symbols::DocSymbolsPickerWidget;
use crate::ui::picker::file::FilePickerWidget;
use crate::ui::picker::grep::GrepPickerWidget;
use crate::ui::picker::references::ReferencesPickerWidget;

/// Transient overlays and dialogs owned by the layout.
#[derive(Debug)]
pub(in crate::layout) struct Dialogs {
    pub command_line: CommandLineState,
    pub command_line_open: bool,
    pub completion: Option<CompletionWidget>,
    pub lsp_rename_prompt: Option<LspRenamePrompt>,
    pub colorscheme_picker: Option<ColorschemePickerWidget>,
    pub code_actions_picker: Option<CodeActionsPickerWidget>,
    pub doc_symbols_picker: Option<DocSymbolsPickerWidget>,
    pub workspace_symbols_picker: Option<DocSymbolsPickerWidget>,
    pub references_picker: Option<ReferencesPickerWidget>,
    pub file_picker: Option<FilePickerWidget>,
    pub grep_picker: Option<GrepPickerWidget>,
    pub confirmation_box: Option<ConfirmationBox>,
    pub hover: Option<HoverWidget>,
    pub diagnostic_hover: Option<DiagnosticHoverWidget>,
}

impl Default for Dialogs {
    fn default() -> Self {
        Self {
            command_line: CommandLineState::new(),
            command_line_open: false,
            completion: None,
            lsp_rename_prompt: None,
            colorscheme_picker: None,
            code_actions_picker: None,
            doc_symbols_picker: None,
            workspace_symbols_picker: None,
            references_picker: None,
            file_picker: None,
            grep_picker: None,
            confirmation_box: None,
            hover: None,
            diagnostic_hover: None,
        }
    }
}

impl Dialogs {
    pub fn completion_mut(&mut self) -> Option<&mut CompletionWidget> {
        self.completion.as_mut()
    }

    pub fn close_all(&mut self) {
        self.command_line_open = false;
        self.command_line.set_cursor(None);
        if let Some(completion) = self.completion.as_mut() {
            completion.close();
        }
        self.completion = None;
        self.colorscheme_picker = None;
        self.code_actions_picker = None;
        self.doc_symbols_picker = None;
        self.workspace_symbols_picker = None;
        self.references_picker = None;
        self.file_picker = None;
        self.grep_picker = None;
        self.confirmation_box = None;
        self.hover = None;
        self.diagnostic_hover = None;
        self.lsp_rename_prompt = None;
    }
}
