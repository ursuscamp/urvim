//! LSP document symbol picker source and selection behavior.

use crate::background::JobManager;
use crate::background::{JobContext, JobEvent, JobKind, JobPayload, JobToken};
use crate::buffer::{BufferId, Cursor};
use crate::globals;
use crate::lsp::runtime::{DocumentSymbolItem, DocumentSymbolTree};
use crate::terminal::Style;
use crate::ui::inputs::PromptSegment;
use crate::ui::line_format::{
    EllipsisPlacement, FormattedLineSection, FormattedLineTemplate, LineSectionAlignment,
    LineSectionOverflow,
};
use crate::ui::picker::{
    PickerItem, PickerPreview, PickerPreviewEvent, PickerRenderSegment, PickerSearchEvent,
    PickerSource, PickerWidget, picker_indicator_glyph,
};
use crate::ui::{Command, Intent};
use lsp_types::SymbolKind;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::thread;

const DOC_SYMBOL_PREVIEW_CONTEXT_LINES: usize = 100;
static NEXT_DOC_SYMBOLS_PICKER_GENERATION: AtomicU64 = AtomicU64::new(1);

/// Scope for the symbols picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocSymbolsPickerScope {
    /// Symbol picker for the active document.
    Document(BufferId),
    /// Symbol picker for the entire workspace.
    Workspace,
}

/// A document symbol displayed by the LSP picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocSymbolsPickerItem {
    path: PathBuf,
    /// Resolved cursor for document symbols, or a placeholder for workspace symbols.
    cursor: Cursor,
    /// Raw LSP range used to lazily resolve workspace-symbol positions.
    range: Option<lsp_types::Range>,
    kind: SymbolKind,
    name: String,
    depth: usize,
    show_path: bool,
}

impl DocSymbolsPickerItem {
    /// Creates a picker item from a resolved LSP document symbol.
    pub fn new(value: &DocumentSymbolItem) -> Self {
        Self {
            path: value.path.clone(),
            cursor: value.cursor,
            range: value.range.clone(),
            kind: value.kind,
            name: value.name.clone(),
            depth: value.depth,
            show_path: false,
        }
    }

    /// Creates a workspace-symbol picker item.
    pub fn new_workspace(value: &DocumentSymbolItem) -> Self {
        Self {
            path: value.path.clone(),
            cursor: value.cursor,
            range: value.range.clone(),
            kind: value.kind,
            name: value.name.clone(),
            depth: value.depth,
            show_path: true,
        }
    }
}

/// Picker source for document symbols in the active buffer.
#[derive(Debug, Clone)]
pub struct DocSymbolsPickerSource {
    scope: DocSymbolsPickerScope,
    query_fuzzy: Arc<AtomicBool>,
    current_generation: Arc<AtomicU64>,
    preview_generation: Arc<AtomicU64>,
    jobs: Arc<JobManager>,
}

/// Document symbol picker query mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    /// Exact substring search.
    Exact,
    /// Fuzzy subsequence search.
    Fuzzy,
}

/// Document symbol picker search job.
#[derive(Debug)]
pub struct DocSymbolsPickerSearchJob {
    scope: DocSymbolsPickerScope,
    query: String,
    query_mode: QueryMode,
}

/// Concrete document symbols picker widget.
pub type DocSymbolsPickerWidget = PickerWidget<DocSymbolsPickerSource>;

impl DocSymbolsPickerSource {
    /// Creates a document symbol picker for the given buffer.
    pub fn with_document_jobs(buffer_id: BufferId, jobs: Arc<JobManager>) -> Self {
        Self::with_scope(DocSymbolsPickerScope::Document(buffer_id), jobs)
    }

    /// Creates a workspace symbol picker.
    pub fn with_workspace_jobs(jobs: Arc<JobManager>) -> Self {
        Self::with_scope(DocSymbolsPickerScope::Workspace, jobs)
    }

    fn with_scope(scope: DocSymbolsPickerScope, jobs: Arc<JobManager>) -> Self {
        Self {
            scope,
            query_fuzzy: Arc::new(AtomicBool::new(false)),
            current_generation: Arc::new(AtomicU64::new(
                NEXT_DOC_SYMBOLS_PICKER_GENERATION.fetch_add(1, Ordering::SeqCst),
            )),
            preview_generation: Arc::new(AtomicU64::new(0)),
            jobs,
        }
    }

    /// Returns the current query mode.
    pub fn query_mode(&self) -> QueryMode {
        if self.query_fuzzy.load(Ordering::SeqCst) {
            QueryMode::Fuzzy
        } else {
            QueryMode::Exact
        }
    }

    /// Updates the current query mode.
    pub fn set_query_mode(&self, mode: QueryMode) {
        self.query_fuzzy
            .store(matches!(mode, QueryMode::Fuzzy), Ordering::SeqCst);
    }

    fn job_kind(&self) -> JobKind {
        match self.scope {
            DocSymbolsPickerScope::Document(_) => JobKind::DocSymbolsPickerSearch,
            DocSymbolsPickerScope::Workspace => JobKind::WorkspaceSymbolsPickerSearch,
        }
    }

    /// Toggles between exact and fuzzy query mode.
    pub fn toggle_query_mode(&self) -> QueryMode {
        let next = match self.query_mode() {
            QueryMode::Exact => QueryMode::Fuzzy,
            QueryMode::Fuzzy => QueryMode::Exact,
        };
        self.set_query_mode(next);
        next
    }

    /// Returns picker prompt segments.
    pub fn query_prompt_segments(mode: QueryMode) -> Vec<PromptSegment> {
        vec![
            PromptSegment::new(
                match mode {
                    QueryMode::Exact => "Exact",
                    QueryMode::Fuzzy => "Fuzzy",
                },
                highlight_style(match mode {
                    QueryMode::Exact => "ui.input.prompt.exact",
                    QueryMode::Fuzzy => "ui.input.prompt.fuzzy",
                }),
            ),
            PromptSegment::new(
                format!(" {} ", picker_indicator_glyph()),
                highlight_style("ui.input.prompt.separator"),
            ),
        ]
    }
}

impl PickerSource for DocSymbolsPickerSource {
    type Item = DocSymbolsPickerItem;

    fn set_generation(&self, generation: u64) {
        self.current_generation.store(generation, Ordering::SeqCst);
    }

    fn job_manager(&self) -> Arc<JobManager> {
        Arc::clone(&self.jobs)
    }

    fn start_search(
        &self,
        query: &str,
        generation: u64,
        _sender: Sender<PickerSearchEvent<Self::Item>>,
    ) {
        let current_generation = self.current_generation.load(Ordering::SeqCst);
        debug_assert_eq!(current_generation, generation);

        let previous_generation = current_generation.saturating_sub(1);
        if previous_generation > 0 {
            self.jobs
                .abort_generation(self.job_kind(), JobToken::new(previous_generation));
        }

        let token = JobToken::new(generation);
        let _ = self.jobs.submit_latest_only(
            self.job_kind(),
            token,
            DocSymbolsPickerSearchJob {
                scope: self.scope,
                query: query.to_string(),
                query_mode: self.query_mode(),
            },
        );
    }

    fn preview_key(&self, item: &Self::Item) -> Option<String> {
        Some(item.path.to_string_lossy().into_owned())
    }

    fn start_preview(&self, item: Self::Item, generation: u64, sender: Sender<PickerPreviewEvent>) {
        self.preview_generation.store(generation, Ordering::SeqCst);
        let current_generation = self.preview_generation.clone();
        thread::spawn(move || {
            sender.send(PickerPreviewEvent::Started { generation }).ok();
            let result = build_document_symbol_preview(&item);
            if current_generation.load(Ordering::SeqCst) != generation {
                return;
            }

            match result {
                Ok(preview) => sender
                    .send(PickerPreviewEvent::Loaded {
                        generation,
                        preview,
                    })
                    .ok(),
                Err(error) => sender
                    .send(PickerPreviewEvent::Failed {
                        generation,
                        message: error.to_string(),
                    })
                    .ok(),
            };
        });
    }

    fn cancel_preview(&self) {
        self.preview_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn select(&self, item: &Self::Item) -> Intent {
        Intent::Command(Command::OpenFileAtCursor(
            item.path.clone(),
            item.resolved_cursor(),
        ))
    }

    fn cancel_search(&self) {
        let generation = self.current_generation.load(Ordering::SeqCst);
        if generation == 0 {
            return;
        }

        self.jobs
            .abort_generation(self.job_kind(), JobToken::new(generation));
    }
}

impl DocSymbolsPickerSearchJob {
    /// Creates a document symbol picker search job.
    pub fn new(scope: DocSymbolsPickerScope, query: String, query_mode: QueryMode) -> Self {
        Self {
            scope,
            query,
            query_mode,
        }
    }

    /// Runs the document symbol picker search job on the worker thread.
    pub fn run(self, context: &JobContext, event_tx: &std::sync::mpsc::Sender<JobEvent>) {
        let _ = event_tx.send(JobEvent::Started {
            kind: context.kind().clone(),
            token: context.token(),
        });

        let mut completed_payload = None;

        if context.is_stopping() || context.is_aborted() {
            return;
        }

        match self.scope {
            DocSymbolsPickerScope::Document(buffer_id) => {
                let result = globals::with_lsp_runtime_mut(|runtime| {
                    runtime.document_symbols_tree_buffer(buffer_id)
                })
                .ok_or_else(|| "LSP runtime is not available".to_string())
                .and_then(|result| result)
                .ok()
                .flatten();

                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                if let Some(nodes) = result {
                    let filtered = filter_document_symbol_trees(
                        nodes,
                        self.query.as_str(),
                        matches!(self.query_mode, QueryMode::Fuzzy),
                    );
                    let items = flatten_document_symbol_trees(filtered);
                    completed_payload = Some(JobPayload::DocSymbolsSearch(
                        items
                            .iter()
                            .cloned()
                            .map(|item| DocSymbolsPickerItem::new(&item))
                            .collect(),
                    ));
                }
            }
            DocSymbolsPickerScope::Workspace => {
                let result = globals::with_lsp_runtime_mut(|runtime| {
                    runtime.workspace_symbols(self.query.as_str())
                })
                .ok_or_else(|| "LSP runtime is not available".to_string())
                .and_then(|result| result)
                .ok()
                .flatten();

                if context.is_stopping() || context.is_aborted() {
                    return;
                }

                if let Some(items) = result {
                    let items = filter_document_symbol_items(
                        items,
                        self.query.as_str(),
                        matches!(self.query_mode, QueryMode::Fuzzy),
                    );
                    completed_payload = Some(JobPayload::DocSymbolsSearch(
                        items
                            .iter()
                            .cloned()
                            .map(|item| DocSymbolsPickerItem::new_workspace(&item))
                            .collect(),
                    ));
                }
            }
        }

        let _ = event_tx.send(JobEvent::Completed {
            kind: context.kind().clone(),
            token: context.token(),
            payload: completed_payload,
        });
    }
}

fn filter_document_symbol_trees(
    nodes: Vec<DocumentSymbolTree>,
    query: &str,
    fuzzy: bool,
) -> Vec<DocumentSymbolTree> {
    nodes
        .into_iter()
        .filter_map(|node| filter_document_symbol_tree(node, query, fuzzy))
        .collect()
}

fn filter_document_symbol_tree(
    node: DocumentSymbolTree,
    query: &str,
    fuzzy: bool,
) -> Option<DocumentSymbolTree> {
    if query.is_empty() {
        return Some(node);
    }

    let children = filter_document_symbol_trees(node.children, query, fuzzy);
    if document_symbol_matches(node.item.search_text.as_str(), query, fuzzy) || !children.is_empty()
    {
        Some(DocumentSymbolTree {
            item: node.item,
            children,
        })
    } else {
        None
    }
}

fn flatten_document_symbol_trees(nodes: Vec<DocumentSymbolTree>) -> Vec<DocumentSymbolItem> {
    let mut items = Vec::new();
    flatten_document_symbol_trees_into(nodes, &mut items);
    items
}

fn filter_document_symbol_items(
    items: Vec<DocumentSymbolItem>,
    query: &str,
    fuzzy: bool,
) -> Vec<DocumentSymbolItem> {
    items
        .into_iter()
        .filter(|item| document_symbol_matches(item.search_text.as_str(), query, fuzzy))
        .collect()
}

fn flatten_document_symbol_trees_into(
    nodes: Vec<DocumentSymbolTree>,
    items: &mut Vec<DocumentSymbolItem>,
) {
    for node in nodes {
        items.push(node.item);
        flatten_document_symbol_trees_into(node.children, items);
    }
}

fn document_symbol_matches(search_text: &str, query: &str, fuzzy: bool) -> bool {
    if fuzzy {
        fuzzy_matches(query, search_text)
    } else {
        exact_matches(query, search_text)
    }
}

fn exact_matches(query: &str, candidate: &str) -> bool {
    candidate
        .to_lowercase()
        .contains(query.to_lowercase().as_str())
}

fn fuzzy_matches(query: &str, candidate: &str) -> bool {
    let mut query_chars = query.chars().flat_map(char::to_lowercase);
    let Some(mut needle) = query_chars.next() else {
        return true;
    };

    for hay in candidate.chars().flat_map(char::to_lowercase) {
        if hay == needle {
            match query_chars.next() {
                Some(next) => needle = next,
                None => return true,
            }
        }
    }

    false
}

impl PickerItem for DocSymbolsPickerItem {
    fn render_segments(
        &self,
        available_cols: usize,
        base_style: Style,
    ) -> Vec<PickerRenderSegment> {
        if self.show_path {
            return self.render_workspace_segments(available_cols, base_style);
        }

        let cursor = self.resolved_cursor();
        let suffix = format!(":{}:{}", cursor.line + 1, cursor.col + 1);
        let suffix_cols = unicode_width::UnicodeWidthStr::width(suffix.as_str());
        if available_cols <= suffix_cols {
            let (visible_suffix, _) =
                crate::ui::picker::visible_tail_text(suffix.as_str(), available_cols, true);
            return vec![PickerRenderSegment::new(
                visible_suffix,
                base_style.faint().accent(location_style()),
            )];
        }

        let mut segments = Vec::new();
        let mut remaining_cols = available_cols.saturating_sub(suffix_cols);
        let prefix = "  ".repeat(self.depth);
        if !prefix.is_empty() {
            let prefix_cols = unicode_width::UnicodeWidthStr::width(prefix.as_str());
            if remaining_cols > prefix_cols {
                segments.push(PickerRenderSegment::new(prefix, base_style));
                remaining_cols = remaining_cols.saturating_sub(prefix_cols);
            }
        }

        if let Some(glyph) = symbol_kind_glyph(self.kind) {
            let glyph_cols = unicode_width::UnicodeWidthStr::width(glyph);
            if remaining_cols > glyph_cols + 1 {
                segments.push(PickerRenderSegment::new(
                    glyph,
                    base_style.accent(accent_style()),
                ));
                segments.push(PickerRenderSegment::new(" ", base_style));
                remaining_cols = remaining_cols.saturating_sub(glyph_cols + 1);
            }
        }

        let (visible_name, _) =
            crate::ui::picker::visible_tail_text(self.name.as_str(), remaining_cols, true);
        segments.push(PickerRenderSegment::new(
            visible_name,
            base_style.overlay(symbol_kind_style(self.kind)),
        ));

        segments.push(PickerRenderSegment::new(
            suffix,
            base_style.faint().accent(location_style()),
        ));
        segments
    }

    fn pad_to_full_width(&self) -> bool {
        !self.show_path
    }
}

impl DocSymbolsPickerItem {
    fn resolved_cursor(&self) -> Cursor {
        let Some(range) = self.range.as_ref() else {
            return self.cursor;
        };

        let Ok(contents) = std::fs::read_to_string(self.path.as_path()) else {
            return self.cursor;
        };
        let lines = crate::buffer::Buffer::from_str(contents.as_str()).line_texts();
        cursor_from_range_utf16(&lines, range).unwrap_or(self.cursor)
    }

    fn render_workspace_segments(
        &self,
        available_cols: usize,
        base_style: Style,
    ) -> Vec<PickerRenderSegment> {
        let (line, col) = self.display_position();
        let suffix = format!(":{}:{}", line + 1, col + 1);
        let path_label = self.path_display();
        let mut sections = Vec::new();
        let mut values = Vec::new();

        if self.depth > 0 {
            sections.push(FormattedLineSection::measured(base_style));
            values.push("  ".repeat(self.depth));
        }

        if let Some(glyph) = symbol_kind_glyph(self.kind) {
            sections.push(FormattedLineSection::measured(
                base_style.accent(accent_style()),
            ));
            values.push(glyph.to_string());
            sections.push(FormattedLineSection::measured(base_style));
            values.push(" ".to_string());
        }

        sections.push(FormattedLineSection::measured(
            base_style.overlay(symbol_kind_style(self.kind)),
        ));
        values.push(self.name.clone());

        sections.push(
            FormattedLineSection::flex(1, base_style)
                .with_alignment(LineSectionAlignment::Right)
                .with_overflow(LineSectionOverflow::Ellipsis(EllipsisPlacement::Start)),
        );
        values.push(path_label);

        sections.push(FormattedLineSection::measured(
            base_style.faint().accent(location_style()),
        ));
        values.push(suffix);

        let rendered = FormattedLineTemplate::new(sections)
            .render_segments(
                values.iter().map(String::as_str),
                available_cols.min(u16::MAX as usize) as u16,
            )
            .expect("workspace symbol picker line template");

        rendered
            .into_iter()
            .map(|segment| PickerRenderSegment::new(segment.text, segment.style))
            .collect()
    }

    fn path_display(&self) -> String {
        let Ok(cwd) = std::env::current_dir() else {
            return self.path.to_string_lossy().into_owned();
        };

        self.path
            .strip_prefix(cwd)
            .unwrap_or(self.path.as_path())
            .to_string_lossy()
            .into_owned()
    }

    fn display_position(&self) -> (usize, usize) {
        if let Some(range) = self.range.as_ref() {
            (range.start.line as usize, range.start.character as usize)
        } else {
            (self.cursor.line, self.cursor.col)
        }
    }
}

fn cursor_from_range_utf16(
    lines: &imbl::Vector<Arc<str>>,
    range: &lsp_types::Range,
) -> Option<Cursor> {
    let line = lines.get(range.start.line as usize)?;
    let col = position_character_to_byte_index(line.as_ref(), range.start.character)?;
    Some(Cursor::new(range.start.line as usize, col))
}

fn position_character_to_byte_index(line: &str, character: u32) -> Option<usize> {
    let target = character as usize;
    let mut units = 0usize;
    for (offset, ch) in line.char_indices() {
        if units == target {
            return Some(offset);
        }
        units = units.saturating_add(ch.len_utf16());
        if units > target {
            return None;
        }
    }

    if units == target {
        Some(line.len())
    } else {
        None
    }
}

fn build_document_symbol_preview(item: &DocSymbolsPickerItem) -> std::io::Result<PickerPreview> {
    let (line, _) = item.display_position();
    let start_line = line.saturating_sub(DOC_SYMBOL_PREVIEW_CONTEXT_LINES);
    let _ = std::fs::metadata(item.path.as_path())?;

    Ok(PickerPreview::new(
        item.path.to_string_lossy(),
        start_line + 1,
        Some(line + 1),
    ))
}

fn highlight_style(name: &str) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(name))
            .unwrap_or_default()
    })
}

fn location_style() -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.resolve_name_with_default("ui.picker.location"))
            .unwrap_or_default()
    })
}

fn accent_style() -> Style {
    globals::with_active_theme(|theme| {
        theme
            .and_then(|theme| {
                theme
                    .highlight_style_for_name("ui.input.prompt")
                    .foreground()
            })
            .map(|color| Style::new().fg(color))
            .unwrap_or_default()
    })
}

fn symbol_kind_style(kind: SymbolKind) -> Style {
    globals::with_active_theme(|theme| {
        theme
            .map(|theme| theme.highlight_style_for_name(symbol_kind_style_name(kind)))
            .unwrap_or_default()
    })
}

fn symbol_kind_style_name(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::FILE | SymbolKind::MODULE | SymbolKind::NAMESPACE | SymbolKind::PACKAGE => {
            "syntax.namespace"
        }
        SymbolKind::CLASS
        | SymbolKind::STRUCT
        | SymbolKind::INTERFACE
        | SymbolKind::ENUM
        | SymbolKind::TYPE_PARAMETER => "syntax.type",
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR => "syntax.function",
        SymbolKind::PROPERTY | SymbolKind::FIELD | SymbolKind::VARIABLE => "syntax.variable",
        SymbolKind::CONSTANT | SymbolKind::ENUM_MEMBER => "syntax.constant",
        SymbolKind::STRING => "syntax.string",
        SymbolKind::NUMBER
        | SymbolKind::BOOLEAN
        | SymbolKind::ARRAY
        | SymbolKind::OBJECT
        | SymbolKind::NULL => "syntax.number",
        SymbolKind::KEY | SymbolKind::EVENT => "syntax.keyword",
        SymbolKind::OPERATOR => "syntax.operator",
        _ => "syntax.variable",
    }
}

fn symbol_kind_glyph(kind: SymbolKind) -> Option<&'static str> {
    if !globals::with_config(|config| config.nerdfont_enabled()).unwrap_or(false) {
        return None;
    }

    Some(match kind {
        SymbolKind::FILE => "",
        SymbolKind::MODULE | SymbolKind::NAMESPACE | SymbolKind::PACKAGE => "",
        SymbolKind::CLASS | SymbolKind::STRUCT => "",
        SymbolKind::INTERFACE => "",
        SymbolKind::ENUM => "",
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR => "",
        SymbolKind::PROPERTY | SymbolKind::FIELD | SymbolKind::VARIABLE | SymbolKind::CONSTANT => {
            ""
        }
        SymbolKind::STRING
        | SymbolKind::NUMBER
        | SymbolKind::BOOLEAN
        | SymbolKind::ARRAY
        | SymbolKind::OBJECT
        | SymbolKind::KEY
        | SymbolKind::NULL
        | SymbolKind::ENUM_MEMBER => "",
        SymbolKind::EVENT | SymbolKind::OPERATOR => "",
        SymbolKind::TYPE_PARAMETER => "",
        _ => "",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{JobEvent, JobKind, JobManager, JobPayload, JobToken};
    use crate::buffer::Cursor;
    use crate::config::Config;
    use crate::globals;
    use crate::lsp::runtime::DocumentSymbolItem;
    use crate::terminal::Style;
    use std::collections::BTreeSet;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "urvim-doc-symbols-picker-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn test_item() -> DocSymbolsPickerItem {
        DocSymbolsPickerItem {
            path: PathBuf::from("/tmp/example.rs"),
            cursor: Cursor::new(7, 3),
            range: None,
            kind: SymbolKind::FUNCTION,
            name: "example_function".to_string(),
            depth: 1,
            show_path: false,
        }
    }

    #[test]
    fn doc_symbols_picker_selects_open_file_at_cursor_intent() {
        let source = DocSymbolsPickerSource::with_document_jobs(
            crate::buffer::BufferId::new(1),
            Arc::new(JobManager::new()),
        );
        let intent = source.select(&test_item());

        assert!(matches!(
            intent,
            Intent::Command(Command::OpenFileAtCursor(_, _))
        ));
    }

    #[test]
    fn doc_symbols_picker_item_renders_location_suffix() {
        let segments = test_item().render_segments(24, Style::default());

        assert!(!segments.is_empty());
        assert!(segments.iter().any(|segment| segment.text.contains(":8:4")));
    }

    #[test]
    fn workspace_symbols_picker_item_uses_line_format_for_workspace_rows() {
        let cwd = std::env::current_dir().unwrap();
        let item = DocSymbolsPickerItem {
            path: cwd.join("very/long/example/path/with/a/lot/of/components/example.rs"),
            cursor: Cursor::new(7, 3),
            range: None,
            kind: SymbolKind::FUNCTION,
            name: "example_function".to_string(),
            depth: 1,
            show_path: true,
        };

        let rendered = item.render_segments(60, Style::default());
        let text = rendered
            .iter()
            .map(|segment| segment.text.as_str())
            .collect::<String>();

        assert!(text.starts_with("  "));
        assert!(text.contains("example_function"));
        assert!(text.contains("example.rs"));
        assert!(text.contains(":8:4"));
        assert!(text.contains("…"));
    }

    #[test]
    fn doc_symbols_picker_item_uses_nerdfont_icon_when_enabled() {
        let _config_guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([crate::config::AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        let segments = test_item().render_segments(24, Style::default());
        assert!(segments.iter().any(|segment| segment.text == ""));
    }

    #[test]
    fn doc_symbols_picker_preview_tracks_the_symbol_line() {
        let temp_root = unique_temp_dir();
        fs::create_dir_all(&temp_root).unwrap();
        let file_path = temp_root.join("example.rs");
        fs::write(&file_path, "one\ntwo\nthree\nfour\nfive\n").unwrap();
        let item = DocSymbolsPickerItem {
            path: file_path,
            cursor: Cursor::new(9, 0),
            range: None,
            kind: SymbolKind::FUNCTION,
            name: "example_function".to_string(),
            depth: 0,
            show_path: false,
        };

        let preview = build_document_symbol_preview(&item).expect("preview");
        assert_eq!(
            preview.start_line,
            10 - DOC_SYMBOL_PREVIEW_CONTEXT_LINES.min(9)
        );
        assert_eq!(preview.highlighted_line, Some(10));

        let _ = fs::remove_dir_all(temp_root);
    }

    #[test]
    fn doc_symbols_picker_item_from_runtime_item_copies_cursor_data() {
        let item = DocumentSymbolItem {
            path: PathBuf::from("/tmp/example.rs"),
            cursor: Cursor::new(3, 7),
            range: None,
            kind: SymbolKind::CLASS,
            name: "Example".to_string(),
            detail: Some("struct Example".to_string()),
            depth: 2,
            search_text: "example class struct example".to_string(),
        };

        let picker_item = DocSymbolsPickerItem::new(&item);
        assert_eq!(picker_item.cursor, Cursor::new(3, 7));
        assert_eq!(picker_item.depth, 2);
    }

    #[test]
    fn doc_symbols_picker_item_uses_syntax_style_for_symbol_kind() {
        let mut highlights = crate::theme::HighlightStyles::default();
        highlights.insert(
            crate::theme::Tag::parse("syntax.function").expect("valid tag"),
            Style::new().fg(crate::terminal::Color::ansi(42)).bold(),
        );
        highlights.insert(
            crate::theme::Tag::parse("ui.input.prompt").expect("valid tag"),
            Style::new().fg(crate::terminal::Color::ansi(20)),
        );
        let _guard = globals::set_test_active_theme(crate::theme::Theme::new(
            "demo",
            crate::theme::ThemeKind::Ansi256,
            Style::new()
                .fg(crate::terminal::Color::ansi(9))
                .bg(crate::terminal::Color::ansi(1)),
            highlights,
        ));
        let _config_guard = globals::set_test_config(Config {
            advanced_glyphs: BTreeSet::from([crate::config::AdvancedGlyphCapability::Nerdfont]),
            ..Config::default()
        });

        let item = DocSymbolsPickerItem {
            path: PathBuf::from("/tmp/example.rs"),
            cursor: Cursor::new(7, 3),
            range: None,
            kind: SymbolKind::FUNCTION,
            name: "example_function".to_string(),
            depth: 0,
            show_path: false,
        };

        let segments = item.render_segments(24, Style::new().bg(crate::terminal::Color::ansi(12)));
        assert_eq!(
            segments[0].style.foreground(),
            Some(crate::terminal::Color::ansi(20))
        );
        assert_eq!(
            segments[2].style,
            Style::new()
                .bg(crate::terminal::Color::ansi(12))
                .fg(crate::terminal::Color::ansi(42))
                .bold()
        );
    }

    #[test]
    fn doc_symbols_picker_query_prompt_segments_follow_mode() {
        let exact = DocSymbolsPickerSource::query_prompt_segments(QueryMode::Exact);
        let fuzzy = DocSymbolsPickerSource::query_prompt_segments(QueryMode::Fuzzy);

        assert_eq!(exact[0].text, "Exact");
        assert_eq!(fuzzy[0].text, "Fuzzy");
    }

    #[test]
    fn doc_symbols_picker_toggles_query_mode() {
        let source = DocSymbolsPickerSource::with_document_jobs(
            crate::buffer::BufferId::new(1),
            Arc::new(JobManager::new()),
        );

        assert_eq!(source.query_mode(), QueryMode::Exact);
        assert_eq!(source.toggle_query_mode(), QueryMode::Fuzzy);
        assert_eq!(source.query_mode(), QueryMode::Fuzzy);
        source.set_query_mode(QueryMode::Exact);
        assert_eq!(source.query_mode(), QueryMode::Exact);
    }

    #[test]
    fn doc_symbols_picker_search_runs_through_the_job_manager() {
        let manager = JobManager::new();
        manager
            .submit(
                JobKind::DocSymbolsPickerSearch,
                JobToken::new(1),
                DocSymbolsPickerSearchJob::new(
                    DocSymbolsPickerScope::Document(crate::buffer::BufferId::new(1)),
                    "example".to_string(),
                    QueryMode::Exact,
                ),
            )
            .unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut saw_started = false;
        let mut saw_complete = false;

        while !saw_complete {
            let _ = manager.process_events(|event| match event {
                JobEvent::Started { kind, .. } if kind == JobKind::DocSymbolsPickerSearch => {
                    saw_started = true;
                }
                JobEvent::Completed {
                    kind,
                    payload: Some(JobPayload::DocSymbolsSearch(_)),
                    ..
                } if kind == JobKind::DocSymbolsPickerSearch => {
                    saw_complete = true;
                }
                JobEvent::Completed { kind, .. } | JobEvent::Failed { kind, .. }
                    if kind == JobKind::DocSymbolsPickerSearch =>
                {
                    saw_complete = true;
                }
                _ => {}
            });

            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for doc symbol job"
            );
            if !saw_complete {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        assert!(saw_started);
        manager.shutdown();
    }

    #[test]
    fn workspace_symbols_picker_search_runs_through_the_job_manager() {
        let manager = JobManager::new();
        manager
            .submit(
                JobKind::WorkspaceSymbolsPickerSearch,
                JobToken::new(1),
                DocSymbolsPickerSearchJob::new(
                    DocSymbolsPickerScope::Workspace,
                    "example".to_string(),
                    QueryMode::Exact,
                ),
            )
            .unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut saw_started = false;
        let mut saw_complete = false;

        while !saw_complete {
            let _ = manager.process_events(|event| match event {
                JobEvent::Started { kind, .. } if kind == JobKind::WorkspaceSymbolsPickerSearch => {
                    saw_started = true;
                }
                JobEvent::Completed {
                    kind,
                    payload: Some(JobPayload::DocSymbolsSearch(_)),
                    ..
                } if kind == JobKind::WorkspaceSymbolsPickerSearch => {
                    saw_complete = true;
                }
                JobEvent::Completed { kind, .. } | JobEvent::Failed { kind, .. }
                    if kind == JobKind::WorkspaceSymbolsPickerSearch =>
                {
                    saw_complete = true;
                }
                _ => {}
            });

            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for workspace symbol job"
            );
            if !saw_complete {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        assert!(saw_started);
        manager.shutdown();
    }

    #[test]
    fn workspace_symbols_picker_search_runs_with_an_empty_query() {
        let manager = Arc::new(JobManager::new());
        let source = DocSymbolsPickerSource::with_workspace_jobs(Arc::clone(&manager));
        let mut picker = DocSymbolsPickerWidget::new(source);

        picker.restart_search();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let mut saw_started = false;
        let mut saw_complete = false;

        while !saw_complete {
            let _ = manager.process_events(|event| match event {
                JobEvent::Started { kind, .. } if kind == JobKind::WorkspaceSymbolsPickerSearch => {
                    saw_started = true;
                }
                JobEvent::Completed { kind, .. } | JobEvent::Failed { kind, .. }
                    if kind == JobKind::WorkspaceSymbolsPickerSearch =>
                {
                    saw_complete = true;
                }
                _ => {}
            });

            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for workspace symbol job"
            );
            if !saw_complete {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }

        assert!(saw_started);
        manager.shutdown();
    }
}
