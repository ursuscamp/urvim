use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use std::path::Path;
use urvim::{AbsolutePath, buffer::Buffer};

const VIEWPORT_LINES: usize = 48;

#[derive(Clone, Copy)]
struct Corpus {
    name: &'static str,
    ext: &'static str,
    source: &'static str,
    dirty_needle: &'static str,
    dirty_replacement: &'static str,
}

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    repeat: usize,
}

const CORPORA: [Corpus; 6] = [
    Corpus {
        name: "rust",
        ext: "rs",
        source: include_str!("../src/buffer/tests/syntax/fixtures/rust.rs"),
        dirty_needle: "Option",
        dirty_replacement: "Result",
    },
    Corpus {
        name: "javascript",
        ext: "js",
        source: include_str!("../src/buffer/tests/syntax/fixtures/javascript.js"),
        dirty_needle: "fetch",
        dirty_replacement: "parse",
    },
    Corpus {
        name: "markdown",
        ext: "md",
        source: include_str!("../src/buffer/tests/syntax/fixtures/markdown.md"),
        dirty_needle: "Some(\"hello\")",
        dirty_replacement: "None(\"hello\")",
    },
    Corpus {
        name: "html",
        ext: "html",
        source: include_str!("../src/buffer/tests/syntax/fixtures/html.html"),
        dirty_needle: "image.png",
        dirty_replacement: "script.js",
    },
    Corpus {
        name: "bash",
        ext: "sh",
        source: include_str!("../src/buffer/tests/syntax/fixtures/bash.sh"),
        dirty_needle: "release",
        dirty_replacement: "staging",
    },
    Corpus {
        name: "plain",
        ext: "txt",
        source: "The quick brown fox jumps over the lazy dog.\nPlain text stays plain.\nNo syntax tokens here.\nThis is only a control corpus.",
        dirty_needle: "plain",
        dirty_replacement: "blank",
    },
];

const SCENARIOS: [Scenario; 2] = [
    Scenario {
        name: "sample",
        repeat: 1,
    },
    Scenario {
        name: "large",
        repeat: 32,
    },
];

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

fn build_buffer(corpus: Corpus, scenario: Scenario) -> Buffer {
    let text = repeat_source(corpus.source, scenario.repeat);
    let path_buf = Path::new("/tmp").join(format!(
        "syntax-highlighter-{}-{}.{}",
        corpus.name, scenario.name, corpus.ext
    ));
    let path = AbsolutePath::from_path(path_buf.as_path()).expect("bench path should be absolute");
    Buffer::from_str_with_path(&text, path)
}

fn warm_full_buffer(buffer: &mut Buffer) {
    if buffer.line_count() == 0 {
        return;
    }

    buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
}

fn warm_viewport(buffer: &mut Buffer) {
    if buffer.line_count() == 0 {
        return;
    }

    let target_line = buffer
        .line_count()
        .saturating_sub(1)
        .min(VIEWPORT_LINES.saturating_sub(1));
    buffer.ensure_syntax_through(target_line);
}

fn lookup_checksum(buffer: &Buffer) -> usize {
    if buffer.line_count() == 0 {
        return 0;
    }

    let mut checksum = 0usize;
    let windows = [
        0,
        buffer.line_count() / 2,
        buffer.line_count().saturating_sub(VIEWPORT_LINES),
    ];

    for start_line in windows {
        let end_line = (start_line + VIEWPORT_LINES).min(buffer.line_count());
        for line in start_line..end_line {
            if let Some(spans) = buffer.cached_syntax_spans_for_line_ref(line) {
                checksum ^= spans.len();
                checksum ^= spans.iter().fold(0usize, |acc, span| {
                    acc ^ span.start_byte
                        ^ span.end_byte
                        ^ span.style.as_str().bytes().fold(0usize, |style_acc, byte| {
                            style_acc.wrapping_mul(31).wrapping_add(byte as usize)
                        })
                });
            }
        }
    }

    checksum
}

fn locate_dirty_line(buffer: &Buffer, needle: &str) -> usize {
    (0..buffer.line_count())
        .find(|line_idx| {
            buffer
                .line_at(*line_idx)
                .is_some_and(|line| line.to_string().contains(needle))
        })
        .unwrap_or(0)
}

fn prepare_dirty_buffer(corpus: Corpus, scenario: Scenario) -> Buffer {
    let mut buffer = build_buffer(corpus, scenario);
    warm_full_buffer(&mut buffer);

    if buffer.line_count() == 0 {
        return buffer;
    }

    let dirty_line = locate_dirty_line(&buffer, corpus.dirty_needle);
    if let Some(line_text) = buffer.line_at(dirty_line) {
        let text = line_text.to_string();
        if let Some(start) = text.find(corpus.dirty_needle) {
            let end = start + corpus.dirty_needle.len();
            buffer.remove(
                urvim::buffer::Cursor::new(dirty_line, start),
                urvim::buffer::Cursor::new(dirty_line, end),
            );
            buffer.insert_text(
                urvim::buffer::Cursor::new(dirty_line, start),
                corpus.dirty_replacement,
            );
        }
    }

    buffer
}

fn bench_viewport_warmup(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_highlighter_viewport_warmup");

    for corpus in CORPORA {
        for scenario in SCENARIOS {
            group.bench_function(
                format!("{}/{}/warm_viewport", corpus.name, scenario.name),
                |b| {
                    b.iter_batched(
                        || build_buffer(corpus, scenario),
                        |mut buffer| {
                            warm_viewport(&mut buffer);
                            black_box(buffer.cached_syntax_line_count())
                        },
                        BatchSize::SmallInput,
                    )
                },
            );
        }
    }

    group.finish();
}

fn bench_full_refresh(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_highlighter_full_refresh");

    for corpus in CORPORA {
        for scenario in SCENARIOS {
            group.bench_function(
                format!("{}/{}/warm_full", corpus.name, scenario.name),
                |b| {
                    b.iter_batched(
                        || build_buffer(corpus, scenario),
                        |mut buffer| {
                            warm_full_buffer(&mut buffer);
                            black_box(buffer.syntax_cache_complete())
                        },
                        BatchSize::SmallInput,
                    )
                },
            );
        }
    }

    group.finish();
}

fn bench_hot_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_highlighter_hot_lookup");

    for corpus in CORPORA {
        for scenario in SCENARIOS {
            let mut buffer = build_buffer(corpus, scenario);
            warm_full_buffer(&mut buffer);

            group.bench_function(
                format!("{}/{}/borrowed_spans", corpus.name, scenario.name),
                |b| b.iter(|| black_box(lookup_checksum(black_box(&buffer)))),
            );
        }
    }

    group.finish();
}

fn bench_dirty_rehighlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_highlighter_dirty_rehighlight");

    for corpus in CORPORA {
        for scenario in SCENARIOS {
            group.bench_function(
                format!("{}/{}/edit_then_rewarm", corpus.name, scenario.name),
                |b| {
                    b.iter_batched(
                        || prepare_dirty_buffer(corpus, scenario),
                        |mut buffer| {
                            warm_full_buffer(&mut buffer);
                            black_box(buffer.syntax_cache_complete())
                        },
                        BatchSize::SmallInput,
                    )
                },
            );
        }
    }

    group.finish();
}

fn bench_cold_midline_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_highlighter_cold_midline_lookup");

    for corpus in CORPORA {
        for scenario in SCENARIOS {
            group.bench_function(
                format!("{}/{}/syntax_spans_for_line", corpus.name, scenario.name),
                |b| {
                    b.iter_batched(
                        || build_buffer(corpus, scenario),
                        |mut buffer| {
                            if buffer.line_count() == 0 {
                                return black_box(0usize);
                            }

                            let target_line = buffer.line_count() / 2;
                            let spans = buffer.syntax_spans_for_line(target_line);
                            black_box(spans.as_ref().map_or(0usize, Vec::len))
                        },
                        BatchSize::SmallInput,
                    )
                },
            );
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_viewport_warmup,
    bench_full_refresh,
    bench_hot_lookup,
    bench_dirty_rehighlight,
    bench_cold_midline_lookup
);
criterion_main!(benches);
