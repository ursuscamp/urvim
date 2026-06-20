use std::env;
use std::path::Path;
use std::process::ExitCode;

use urvim_core::buffer::{Buffer, SyntaxSpan, TextRef};

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: cargo run --bin dump_tokens -- <file>");
        return ExitCode::from(2);
    };

    let path = Path::new(&path);
    let mut buffer = match Buffer::load_from_file(path) {
        Ok(buffer) => buffer,
        Err(error) => {
            eprintln!("failed to read {}: {error}", path.display());
            return ExitCode::from(1);
        }
    };

    println!(
        "{{\"file\":{},\"syntax\":{},\"line_count\":{}}}",
        json_string(&path.display().to_string()),
        json_string(buffer.syntax_name()),
        buffer.line_count()
    );

    for line_idx in 0..buffer.line_count() {
        let line = buffer
            .line_at(line_idx)
            .map(|line| line.to_text())
            .unwrap_or_default();
        let spans = buffer.syntax_spans_for_line(line_idx).unwrap_or_default();
        print_line_tokens(line_idx, &line, &spans);
    }

    ExitCode::SUCCESS
}

fn print_line_tokens(line_idx: usize, line: &str, spans: &[SyntaxSpan]) {
    let mut cursor = 0;

    for span in spans {
        if cursor < span.start_byte {
            print_token(line_idx, cursor, span.start_byte, "unstyled", line);
        }

        let start = span.start_byte.min(line.len());
        let end = span.end_byte.min(line.len());
        if start < end {
            print_token(line_idx, start, end, span.style.as_str(), line);
        }
        cursor = cursor.max(end);
    }

    if cursor < line.len() {
        print_token(line_idx, cursor, line.len(), "unstyled", line);
    }

    if line.is_empty() {
        println!(
            "{{\"line\":{},\"start\":0,\"end\":0,\"style\":\"unstyled\",\"text\":\"\"}}",
            line_idx + 1
        );
    }
}

fn print_token(line_idx: usize, start: usize, end: usize, style: &str, line: &str) {
    println!(
        "{{\"line\":{},\"start\":{},\"end\":{},\"style\":{},\"text\":{}}}",
        line_idx + 1,
        start,
        end,
        json_string(style),
        json_string(&line[start..end])
    );
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
