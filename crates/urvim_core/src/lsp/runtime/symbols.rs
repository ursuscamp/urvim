//! Symbol/reference conversion from raw LSP types to core types.
//!
//! Converts `DocumentSymbolResponse` → `Vec<DocumentSymbolItem>` /
//! `Vec<DocumentSymbolTree>`, workspace symbol responses to `DocumentSymbolItem`,
//! and `Vec<Location>` → `Vec<ReferenceItem>`. Uses `position_to_cursor` for
//! LSP-to-cursor conversion and `globals::open_buffer` for reference resolution.

use std::path::PathBuf;

use lsp_types::{
    Location, OneOf, PositionEncodingKind, SymbolInformation, WorkspaceLocation, WorkspaceSymbol,
};

use urvim_text::{Cursor, PieceTable, TextRef};

use crate::globals;

use super::{
    DocumentSymbolItem, DocumentSymbolTree, ReferenceItem, position_to_cursor, uri_to_file_path,
};

pub(super) fn flatten_document_symbol_response(
    response: lsp_types::DocumentSymbolResponse,
    path: PathBuf,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
) -> Vec<DocumentSymbolItem> {
    let nodes = build_document_symbol_nodes(response, path, lines, encoding);
    flatten_document_symbol_nodes(nodes)
}

pub(super) fn build_document_symbol_nodes(
    response: lsp_types::DocumentSymbolResponse,
    path: PathBuf,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
) -> Vec<DocumentSymbolTree> {
    match response {
        lsp_types::DocumentSymbolResponse::Flat(symbols) => symbols
            .into_iter()
            .filter_map(|symbol| {
                let cursor =
                    position_to_cursor(lines, symbol.location.range.start, encoding.clone())?;
                Some(DocumentSymbolTree {
                    item: DocumentSymbolItem {
                        path: path.clone(),
                        cursor,
                        range: None,
                        kind: symbol.kind,
                        name: symbol.name.clone(),
                        detail: None,
                        depth: 0,
                        search_text: document_symbol_search_text(
                            symbol.name.as_str(),
                            None,
                            symbol.kind,
                        ),
                    },
                    children: Vec::new(),
                })
            })
            .collect(),
        lsp_types::DocumentSymbolResponse::Nested(symbols) => {
            build_nested_document_symbol_nodes(symbols, path.as_path(), lines, encoding, &[])
        }
    }
}

fn build_nested_document_symbol_nodes(
    symbols: Vec<lsp_types::DocumentSymbol>,
    path: &std::path::Path,
    lines: &PieceTable,
    encoding: PositionEncodingKind,
    ancestors: &[String],
) -> Vec<DocumentSymbolTree> {
    let mut nodes = Vec::new();

    for symbol in symbols {
        let mut next_ancestors = ancestors.to_vec();
        next_ancestors.push(symbol.name.clone());

        if let Some(cursor) =
            position_to_cursor(lines, symbol.selection_range.start, encoding.clone())
        {
            let children = symbol.children.map_or_else(Vec::new, |children| {
                build_nested_document_symbol_nodes(
                    children,
                    path,
                    lines,
                    encoding.clone(),
                    &next_ancestors,
                )
            });

            nodes.push(DocumentSymbolTree {
                item: DocumentSymbolItem {
                    path: path.to_path_buf(),
                    cursor,
                    range: None,
                    kind: symbol.kind,
                    name: symbol.name.clone(),
                    detail: symbol.detail.clone(),
                    depth: ancestors.len(),
                    search_text: document_symbol_search_text(
                        symbol.name.as_str(),
                        symbol.detail.as_deref(),
                        symbol.kind,
                    ),
                },
                children,
            });
        }
    }

    nodes
}

pub(super) fn flatten_document_symbol_nodes(
    nodes: Vec<DocumentSymbolTree>,
) -> Vec<DocumentSymbolItem> {
    let mut items = Vec::new();
    flatten_document_symbol_nodes_into(nodes, &mut items);
    items
}

fn flatten_document_symbol_nodes_into(
    nodes: Vec<DocumentSymbolTree>,
    items: &mut Vec<DocumentSymbolItem>,
) {
    for node in nodes {
        items.push(node.item);
        flatten_document_symbol_nodes_into(node.children, items);
    }
}

fn document_symbol_search_text(
    name: &str,
    detail: Option<&str>,
    kind: lsp_types::SymbolKind,
) -> String {
    let mut text = String::new();
    text.push_str(name);
    text.push(' ');
    text.push_str(symbol_kind_label(kind));
    if let Some(detail) = detail.filter(|detail| !detail.trim().is_empty()) {
        text.push(' ');
        text.push_str(detail);
    }
    text.to_lowercase()
}

pub(super) fn workspace_symbol_information_to_item(
    symbol: SymbolInformation,
) -> Option<DocumentSymbolItem> {
    let path = uri_to_file_path(symbol.location.uri.as_str()).ok()?;
    let name = symbol.name;
    let container_name = symbol.container_name;
    let kind = symbol.kind;

    Some(DocumentSymbolItem {
        path,
        cursor: Cursor::new(0, 0),
        range: Some(symbol.location.range),
        kind,
        name: name.clone(),
        detail: container_name.clone(),
        depth: 0,
        search_text: workspace_symbol_search_text(name.as_str(), container_name.as_deref(), kind),
    })
}

pub(super) fn workspace_symbol_to_item(symbol: WorkspaceSymbol) -> Option<DocumentSymbolItem> {
    let (uri, range) = match symbol.location {
        OneOf::Left(Location { uri, range }) => (uri, Some(range)),
        OneOf::Right(WorkspaceLocation { uri }) => (uri, None),
    };
    let path = uri_to_file_path(uri.as_str()).ok()?;
    let name = symbol.name;
    let container_name = symbol.container_name;
    let kind = symbol.kind;

    Some(DocumentSymbolItem {
        path,
        cursor: Cursor::new(0, 0),
        range,
        kind,
        name: name.clone(),
        detail: container_name.clone(),
        depth: 0,
        search_text: workspace_symbol_search_text(name.as_str(), container_name.as_deref(), kind),
    })
}

fn workspace_symbol_search_text(
    name: &str,
    container_name: Option<&str>,
    kind: lsp_types::SymbolKind,
) -> String {
    let mut text = String::new();
    text.push_str(name);
    text.push(' ');
    if let Some(container_name) = container_name.filter(|value| !value.trim().is_empty()) {
        text.push_str(container_name);
        text.push(' ');
    }
    text.push_str(symbol_kind_label(kind));
    text.to_lowercase()
}

pub(super) fn locations_to_reference_items(locations: Vec<Location>) -> Vec<ReferenceItem> {
    locations
        .into_iter()
        .filter_map(location_to_reference_item)
        .collect()
}

fn location_to_reference_item(location: Location) -> Option<ReferenceItem> {
    let path = uri_to_file_path(location.uri.as_str()).ok()?;
    let buffer_id = globals::open_buffer(&path).ok()?;
    let lines = globals::with_buffer(buffer_id, |buffer| buffer.text_snapshot())?;
    let cursor = position_to_cursor(&lines, location.range.start, PositionEncodingKind::UTF16)?;
    let line_text = lines
        .get(cursor.line)
        .map(|line| line.to_text().trim().to_string())
        .unwrap_or_default();

    Some(ReferenceItem {
        path,
        cursor,
        line_text,
    })
}

fn symbol_kind_label(kind: lsp_types::SymbolKind) -> &'static str {
    match kind {
        lsp_types::SymbolKind::FILE => "file",
        lsp_types::SymbolKind::MODULE => "module",
        lsp_types::SymbolKind::NAMESPACE => "namespace",
        lsp_types::SymbolKind::PACKAGE => "package",
        lsp_types::SymbolKind::CLASS => "class",
        lsp_types::SymbolKind::METHOD => "method",
        lsp_types::SymbolKind::PROPERTY => "property",
        lsp_types::SymbolKind::FIELD => "field",
        lsp_types::SymbolKind::CONSTRUCTOR => "constructor",
        lsp_types::SymbolKind::ENUM => "enum",
        lsp_types::SymbolKind::INTERFACE => "interface",
        lsp_types::SymbolKind::FUNCTION => "function",
        lsp_types::SymbolKind::VARIABLE => "variable",
        lsp_types::SymbolKind::CONSTANT => "constant",
        lsp_types::SymbolKind::STRING => "string",
        lsp_types::SymbolKind::NUMBER => "number",
        lsp_types::SymbolKind::BOOLEAN => "boolean",
        lsp_types::SymbolKind::ARRAY => "array",
        lsp_types::SymbolKind::OBJECT => "object",
        lsp_types::SymbolKind::KEY => "key",
        lsp_types::SymbolKind::NULL => "null",
        lsp_types::SymbolKind::ENUM_MEMBER => "enum-member",
        lsp_types::SymbolKind::STRUCT => "struct",
        lsp_types::SymbolKind::EVENT => "event",
        lsp_types::SymbolKind::OPERATOR => "operator",
        lsp_types::SymbolKind::TYPE_PARAMETER => "type-parameter",
        _ => "symbol",
    }
}
