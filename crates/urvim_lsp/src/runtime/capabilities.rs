//! Server capability checks.
//!
//! Each method inspects the negotiated server capabilities to determine whether
//! a specific LSP feature is supported. The `inlay_hints_enabled` parameter is
//! passed by the caller (core reads it from config) — the session does not
//! access editor config directly.

use lsp_types::{CodeActionProviderCapability, OneOf};

use super::session::LspServerSession;

impl LspServerSession {
    pub fn supports_hover(&self) -> bool {
        matches!(
            self.negotiated
                .server_capabilities
                .as_ref()
                .and_then(|capabilities| { capabilities.hover_provider.as_ref() }),
            Some(lsp_types::HoverProviderCapability::Simple(true))
                | Some(lsp_types::HoverProviderCapability::Options(_))
        )
    }

    pub fn supports_completion(&self) -> bool {
        self.negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.completion_provider.as_ref())
            .is_some()
    }

    pub fn supports_definition(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.definition_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    pub fn supports_document_symbols(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.document_symbol_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    pub fn supports_references(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.references_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    pub fn supports_workspace_symbols(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.workspace_symbol_provider.as_ref())
        {
            Some(OneOf::Left(enabled)) => *enabled,
            Some(OneOf::Right(_)) => true,
            None => false,
        }
    }

    pub fn supports_rename(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.rename_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    pub fn supports_inlay_hints(&self, inlay_hints_enabled: bool) -> bool {
        self.server_supports_inlay_hints() && inlay_hints_enabled
    }

    pub fn has_active_progress(&self) -> bool {
        self.progress
            .lock()
            .is_ok_and(|progress| progress.has_active_progress())
    }

    pub fn server_supports_inlay_hints(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.inlay_hint_provider.as_ref())
        {
            Some(lsp_types::OneOf::Left(enabled)) => *enabled,
            Some(lsp_types::OneOf::Right(_)) => true,
            None => false,
        }
    }

    pub fn supports_code_actions(&self) -> bool {
        match self
            .negotiated
            .server_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.code_action_provider.as_ref())
        {
            Some(CodeActionProviderCapability::Simple(enabled)) => *enabled,
            Some(CodeActionProviderCapability::Options(_)) => true,
            None => false,
        }
    }

    pub fn supports_prepare_rename(&self) -> bool {
        let Some(capabilities) = self.negotiated.server_capabilities.as_ref() else {
            return false;
        };

        match capabilities.rename_provider.as_ref() {
            Some(lsp_types::OneOf::Right(options)) => options.prepare_provider.unwrap_or(false),
            None => false,
            Some(lsp_types::OneOf::Left(_)) => false,
        }
    }
}
