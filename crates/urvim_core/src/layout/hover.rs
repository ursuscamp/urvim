use super::Layout;
use crate::ui::diagnostic_hover::DiagnosticHoverWidget;
use crate::ui::hover::HoverWidget;
use crate::window::Position;

impl Layout {
    /// Opens an LSP hover popup near the provided cursor position.
    pub(super) fn open_lsp_hover(&mut self, text: String, anchor: Position) {
        self.close_all_dialogs();
        self.dialogs.hover = HoverWidget::new(text, anchor);
    }

    /// Closes any active LSP hover popup.
    pub(super) fn close_hover(&mut self) {
        self.dialogs.hover = None;
    }

    /// Opens a diagnostic popup near the provided cursor position.
    pub(super) fn open_diagnostic_hover(
        &mut self,
        diagnostics: Vec<lsp_types::Diagnostic>,
        anchor: Position,
    ) {
        self.close_all_dialogs();
        self.dialogs.diagnostic_hover = DiagnosticHoverWidget::new(diagnostics, anchor);
    }

    /// Closes any active diagnostic popup.
    pub(super) fn close_diagnostic_hover(&mut self) {
        self.dialogs.diagnostic_hover = None;
    }

    /// Returns true when the diagnostic popup is open.
    pub fn diagnostic_hover_is_open(&self) -> bool {
        self.dialogs
            .diagnostic_hover
            .as_ref()
            .is_some_and(DiagnosticHoverWidget::is_open)
    }

    /// Returns the diagnostic popup when it is open.
    pub(super) fn diagnostic_hover_mut(&mut self) -> Option<&mut DiagnosticHoverWidget> {
        self.dialogs.diagnostic_hover.as_mut()
    }

    /// Returns true when the hover popup is open.
    pub fn hover_is_open(&self) -> bool {
        self.dialogs
            .hover
            .as_ref()
            .is_some_and(HoverWidget::is_open)
    }

    /// Returns the hover popup when it is open.
    pub(super) fn hover_mut(&mut self) -> Option<&mut HoverWidget> {
        self.dialogs.hover.as_mut()
    }
}
