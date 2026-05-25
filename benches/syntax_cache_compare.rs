use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use std::path::Path;
use urvim::{
    AbsolutePath,
    buffer::{Buffer, Cursor, TextRef},
};

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    line_count: usize,
}

const SCENARIOS: [Scenario; 4] = [
    Scenario {
        name: "small_1k",
        line_count: 1_000,
    },
    Scenario {
        name: "moderate_10k",
        line_count: 10_000,
    },
    Scenario {
        name: "dense_10k",
        line_count: 10_000,
    },
    Scenario {
        name: "large_50k",
        line_count: 50_000,
    },
];

fn build_text(scenario: Scenario) -> String {
    let mut text = String::new();
    for line_idx in 0..scenario.line_count {
        if line_idx > 0 {
            text.push('\n');
        }
        text.push_str(&format!(
            "line {line_idx}: let value_{line_idx} = Some(\"text\");"
        ));
    }
    text
}

fn build_current_buffer(scenario: Scenario) -> Buffer {
    let path = AbsolutePath::from_path(Path::new("/tmp/syntax-cache-bench.rs"))
        .expect("bench path should be absolute");
    Buffer::from_str_with_path(&build_text(scenario), path)
}

fn build_warmed_buffer(scenario: Scenario) -> Buffer {
    let mut buffer = build_current_buffer(scenario);
    if buffer.line_count() > 0 {
        buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
    }
    buffer
}

fn bench_syntax_cache_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_cache_build");

    for scenario in SCENARIOS {
        group.bench_function(format!("current/{}/ensure_full", scenario.name), |b| {
            b.iter_batched(
                || build_current_buffer(scenario),
                |mut buffer| {
                    buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
                    black_box(buffer.cached_syntax_line_count())
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_syntax_cache_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_cache_clone");

    for scenario in SCENARIOS {
        let buffer = build_warmed_buffer(scenario);
        group.bench_function(format!("current/{}/clone_buffer", scenario.name), |b| {
            b.iter(|| {
                let cloned = black_box(&buffer).clone();
                black_box(cloned.cached_syntax_line_count())
            })
        });
    }

    group.finish();
}

fn bench_syntax_cache_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_cache_lookup");

    for scenario in SCENARIOS {
        let buffer = build_warmed_buffer(scenario);
        group.bench_function(format!("current/{}/clone_spans", scenario.name), |b| {
            b.iter(|| {
                let checksum = (0..buffer.line_count()).fold(0usize, |acc, line| {
                    let spans = black_box(buffer.cached_syntax_spans_for_line(line));
                    acc ^ spans.map(|spans| spans.len()).unwrap_or_default()
                });
                black_box(checksum)
            })
        });

        group.bench_function(format!("current/{}/borrow_spans", scenario.name), |b| {
            b.iter(|| {
                let checksum = (0..buffer.line_count()).fold(0usize, |acc, line| {
                    let spans = black_box(buffer.cached_syntax_spans_for_line_ref(line));
                    acc ^ spans.map(|spans| spans.len()).unwrap_or_default()
                });
                black_box(checksum)
            })
        });
    }

    group.finish();
}

fn bench_syntax_cache_insert_newline(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_cache_insert_newline");

    for scenario in SCENARIOS {
        let line = scenario.line_count / 3;

        group.bench_function(format!("current/{}/buffer_insert", scenario.name), |b| {
            b.iter_batched(
                || build_warmed_buffer(scenario),
                |mut buffer| {
                    buffer.insert_text(Cursor::new(line, 0), "\n");
                    black_box(buffer.line_count())
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_syntax_cache_delete_newline(c: &mut Criterion) {
    let mut group = c.benchmark_group("syntax_cache_delete_newline");

    for scenario in SCENARIOS {
        let line = scenario.line_count / 3;

        group.bench_function(format!("current/{}/buffer_remove", scenario.name), |b| {
            b.iter_batched(
                || build_warmed_buffer(scenario),
                |mut buffer| {
                    let line_len = buffer.line_at(line).expect("edit line should exist").len();
                    buffer.remove(Cursor::new(line, line_len), Cursor::new(line + 1, 0));
                    black_box(buffer.line_count())
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_syntax_cache_build,
    bench_syntax_cache_clone,
    bench_syntax_cache_lookup,
    bench_syntax_cache_insert_newline,
    bench_syntax_cache_delete_newline
);
criterion_main!(benches);
