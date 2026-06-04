//! HTML tokenizer benchmarks.
//!
//! Run with: cargo bench --bench syntax_tokenizer_html

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use std::path::Path;
use urvim::{AbsolutePath, buffer::Buffer};

/// HTML fixture source.
const HTML_SOURCE: &str = include_str!("../src/buffer/tests/syntax/fixtures/html.html");

/// Repeat the source N times so we get a meaningful work unit.
fn repeat_source(source: &str, repeat: usize) -> String {
    if repeat == 1 {
        return source.to_owned();
    }
    let mut text = String::with_capacity(source.len() * repeat + repeat.saturating_sub(1));
    for idx in 0..repeat {
        if idx > 0 {
            text.push('\n');
        }
        text.push_str(source.trim_end_matches('\n'));
    }
    text
}

fn build_html_buffer(source: &str) -> Buffer {
    let path_buf = Path::new("/tmp").join("syntax-tokenizer-bench.html");
    let path = AbsolutePath::from_path(path_buf.as_path()).expect("bench path");
    Buffer::from_str_with_path(source, path)
}

fn tokenize_all_lines(buffer: &mut Buffer) -> usize {
    if buffer.line_count() == 0 {
        return 0;
    }
    buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
    buffer.cached_syntax_line_count()
}

fn bench_html_full_refresh(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_tokenizer_full_refresh");

    for (name, repeat) in [("sample", 1usize), ("large", 32usize), ("huge", 256usize)] {
        group.bench_function(name.to_string(), |b| {
            b.iter_batched(
                || build_html_buffer(&repeat_source(HTML_SOURCE, repeat)),
                |mut buf| {
                    let lines = tokenize_all_lines(&mut buf);
                    black_box(lines)
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_html_viewport_warmup(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_tokenizer_viewport_warmup");

    for (name, repeat) in [("sample", 1usize), ("large", 32usize), ("huge", 256usize)] {
        group.bench_function(name.to_string(), |b| {
            b.iter_batched(
                || build_html_buffer(&repeat_source(HTML_SOURCE, repeat)),
                |mut buf| {
                    let target = buf.line_count().saturating_sub(1).min(47);
                    buf.ensure_syntax_through(target);
                    black_box(buf.cached_syntax_line_count())
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_html_cold_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_tokenizer_cold_lookup");

    for (name, repeat) in [("sample", 1usize), ("large", 32usize), ("huge", 256usize)] {
        group.bench_function(name.to_string(), |b| {
            b.iter_batched(
                || build_html_buffer(&repeat_source(HTML_SOURCE, repeat)),
                |mut buf| {
                    if buf.line_count() == 0 {
                        return black_box(0usize);
                    }
                    let mid = buf.line_count() / 2;
                    let spans = buf.syntax_spans_for_line(mid);
                    black_box(spans.as_ref().map_or(0, Vec::len))
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_html_full_refresh,
    bench_html_viewport_warmup,
    bench_html_cold_lookup,
);
criterion_main!(benches);
