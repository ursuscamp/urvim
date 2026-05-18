use super::command_line::CommandLineState;
use crate::ui::Intent;
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
use crate::window::Position;

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
    pub fn open_command_line(&mut self) {
        self.command_line_open = true;
        self.command_line.input_widget_mut().set_prompt(":");
        self.command_line.reset_input();
    }

    pub fn close_command_line(&mut self) {
        self.command_line_open = false;
        self.command_line.set_cursor(None);
    }

    pub fn close_completion(&mut self) {
        if let Some(completion) = self.completion.as_mut() {
            completion.close();
        }
        self.completion = None;
    }

    pub fn completion_is_open(&self) -> bool {
        self.completion
            .as_ref()
            .is_some_and(|completion| completion.is_open())
    }

    pub fn completion_mut(&mut self) -> Option<&mut CompletionWidget> {
        self.completion.as_mut()
    }

    pub fn command_line_is_open(&self) -> bool {
        self.command_line_open
    }

    pub fn close_lsp_rename_prompt(&mut self) {
        self.lsp_rename_prompt = None;
    }

    pub fn open_lsp_rename_prompt(&mut self, prompt: LspRenamePrompt) {
        self.lsp_rename_prompt = Some(prompt);
    }

    pub fn lsp_rename_prompt_is_open(&self) -> bool {
        self.lsp_rename_prompt
            .as_ref()
            .is_some_and(LspRenamePrompt::is_open)
    }

    pub fn lsp_rename_prompt_mut(&mut self) -> Option<&mut LspRenamePrompt> {
        self.lsp_rename_prompt.as_mut()
    }

    pub fn open_colorscheme_picker(&mut self, picker: ColorschemePickerWidget) {
        self.colorscheme_picker = Some(picker);
    }

    pub fn close_colorscheme_picker(&mut self) {
        if let Some(picker) = self.colorscheme_picker.as_mut() {
            picker.close();
        }
        self.colorscheme_picker = None;
    }

    pub fn colorscheme_picker_is_open(&self) -> bool {
        self.colorscheme_picker
            .as_ref()
            .is_some_and(ColorschemePickerWidget::is_open)
    }

    pub fn colorscheme_picker_mut(&mut self) -> Option<&mut ColorschemePickerWidget> {
        self.colorscheme_picker.as_mut()
    }

    pub fn open_code_actions_picker(&mut self, picker: CodeActionsPickerWidget) {
        self.code_actions_picker = Some(picker);
    }

    pub fn close_code_actions_picker(&mut self) {
        if let Some(picker) = self.code_actions_picker.as_mut() {
            picker.close();
        }
        self.code_actions_picker = None;
    }

    pub fn code_actions_picker_is_open(&self) -> bool {
        self.code_actions_picker
            .as_ref()
            .is_some_and(CodeActionsPickerWidget::is_open)
    }

    pub fn code_actions_picker_mut(&mut self) -> Option<&mut CodeActionsPickerWidget> {
        self.code_actions_picker.as_mut()
    }

    pub fn open_doc_symbols_picker(&mut self, picker: DocSymbolsPickerWidget) {
        self.doc_symbols_picker = Some(picker);
    }

    pub fn close_doc_symbols_picker(&mut self) {
        if let Some(picker) = self.doc_symbols_picker.as_mut() {
            picker.close();
        }
        self.doc_symbols_picker = None;
    }

    pub fn doc_symbols_picker_is_open(&self) -> bool {
        self.doc_symbols_picker
            .as_ref()
            .is_some_and(DocSymbolsPickerWidget::is_open)
    }

    pub fn doc_symbols_picker_mut(&mut self) -> Option<&mut DocSymbolsPickerWidget> {
        self.doc_symbols_picker.as_mut()
    }

    pub fn open_workspace_symbols_picker(&mut self, picker: DocSymbolsPickerWidget) {
        self.workspace_symbols_picker = Some(picker);
    }

    pub fn close_workspace_symbols_picker(&mut self) {
        if let Some(picker) = self.workspace_symbols_picker.as_mut() {
            picker.close();
        }
        self.workspace_symbols_picker = None;
    }

    pub fn workspace_symbols_picker_is_open(&self) -> bool {
        self.workspace_symbols_picker
            .as_ref()
            .is_some_and(DocSymbolsPickerWidget::is_open)
    }

    pub fn workspace_symbols_picker_mut(&mut self) -> Option<&mut DocSymbolsPickerWidget> {
        self.workspace_symbols_picker.as_mut()
    }

    pub fn open_references_picker(&mut self, picker: ReferencesPickerWidget) {
        self.references_picker = Some(picker);
    }

    pub fn close_references_picker(&mut self) {
        if let Some(picker) = self.references_picker.as_mut() {
            picker.close();
        }
        self.references_picker = None;
    }

    pub fn references_picker_is_open(&self) -> bool {
        self.references_picker
            .as_ref()
            .is_some_and(ReferencesPickerWidget::is_open)
    }

    pub fn references_picker_mut(&mut self) -> Option<&mut ReferencesPickerWidget> {
        self.references_picker.as_mut()
    }

    pub fn open_file_picker(&mut self, picker: FilePickerWidget) {
        self.file_picker = Some(picker);
    }

    pub fn close_file_picker(&mut self) {
        if let Some(picker) = self.file_picker.as_mut() {
            picker.close();
        }
        self.file_picker = None;
    }

    pub fn file_picker_is_open(&self) -> bool {
        self.file_picker
            .as_ref()
            .is_some_and(FilePickerWidget::is_open)
    }

    pub fn file_picker_mut(&mut self) -> Option<&mut FilePickerWidget> {
        self.file_picker.as_mut()
    }

    pub fn open_grep_picker(&mut self, picker: GrepPickerWidget) {
        self.grep_picker = Some(picker);
    }

    pub fn close_grep_picker(&mut self) {
        if let Some(picker) = self.grep_picker.as_mut() {
            picker.close();
        }
        self.grep_picker = None;
    }

    pub fn grep_picker_is_open(&self) -> bool {
        self.grep_picker
            .as_ref()
            .is_some_and(GrepPickerWidget::is_open)
    }

    pub fn grep_picker_mut(&mut self) -> Option<&mut GrepPickerWidget> {
        self.grep_picker.as_mut()
    }

    pub fn open_confirmation_box(
        &mut self,
        query: impl Into<String>,
        positive_intent: impl Into<Intent>,
    ) {
        self.confirmation_box = Some(ConfirmationBox::new(query, positive_intent));
    }

    pub fn close_confirmation_box(&mut self) {
        self.confirmation_box = None;
    }

    pub fn confirmation_box_is_open(&self) -> bool {
        self.confirmation_box
            .as_ref()
            .is_some_and(ConfirmationBox::is_open)
    }

    pub fn confirmation_box_mut(&mut self) -> Option<&mut ConfirmationBox> {
        self.confirmation_box.as_mut()
    }

    pub fn open_hover(&mut self, text: String, anchor: Position) {
        self.hover = HoverWidget::new(text, anchor);
    }

    pub fn close_hover(&mut self) {
        self.hover = None;
    }

    pub fn hover_is_open(&self) -> bool {
        self.hover.as_ref().is_some_and(HoverWidget::is_open)
    }

    pub fn hover_mut(&mut self) -> Option<&mut HoverWidget> {
        self.hover.as_mut()
    }

    pub fn open_diagnostic_hover(
        &mut self,
        diagnostics: Vec<lsp_types::Diagnostic>,
        anchor: Position,
    ) {
        self.diagnostic_hover = DiagnosticHoverWidget::new(diagnostics, anchor);
    }

    pub fn close_diagnostic_hover(&mut self) {
        self.diagnostic_hover = None;
    }

    pub fn diagnostic_hover_is_open(&self) -> bool {
        self.diagnostic_hover
            .as_ref()
            .is_some_and(DiagnosticHoverWidget::is_open)
    }

    pub fn diagnostic_hover_mut(&mut self) -> Option<&mut DiagnosticHoverWidget> {
        self.diagnostic_hover.as_mut()
    }

    pub fn close_all(&mut self) {
        self.close_command_line();
        self.close_completion();
        self.close_colorscheme_picker();
        self.close_code_actions_picker();
        self.close_doc_symbols_picker();
        self.close_workspace_symbols_picker();
        self.close_references_picker();
        self.close_file_picker();
        self.close_grep_picker();
        self.close_confirmation_box();
        self.close_hover();
        self.close_diagnostic_hover();
        self.close_lsp_rename_prompt();
    }
}
