use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use urvim_core::buffer::{Buffer, Cursor};

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    line_count: usize,
    max_depth: usize,
    period: usize,
}

const SCENARIOS: [Scenario; 4] = [
    Scenario {
        name: "flat_10k",
        line_count: 10_000,
        max_depth: 1,
        period: 2,
    },
    Scenario {
        name: "moderate_10k_depth_4",
        line_count: 10_000,
        max_depth: 4,
        period: 12,
    },
    Scenario {
        name: "nested_10k_depth_12",
        line_count: 10_000,
        max_depth: 12,
        period: 36,
    },
    Scenario {
        name: "large_25k_depth_4",
        line_count: 25_000,
        max_depth: 4,
        period: 12,
    },
];

fn depth_for_line(line_idx: usize, scenario: Scenario) -> usize {
    if scenario.max_depth <= 1 {
        return usize::from(line_idx % scenario.period != 0);
    }

    let period = scenario.period.max(2);
    let half = period / 2;
    let phase = line_idx % period;
    let ramp = if phase < half { phase } else { period - phase };
    1 + ramp.min(scenario.max_depth - 1)
}

fn scenario_text(scenario: Scenario) -> String {
    let mut text = String::new();
    for line_idx in 0..scenario.line_count {
        if line_idx > 0 {
            text.push('\n');
        }
        let depth = depth_for_line(line_idx, scenario);
        text.extend(std::iter::repeat_n(' ', depth * 2));
        text.push_str("line_");
        text.push_str(&line_idx.to_string());
    }
    text
}

fn cached_buffer(scenario: Scenario) -> Buffer {
    let text = scenario_text(scenario);
    let mut buffer = Buffer::from_str(&text);
    buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
    buffer
}

fn checksum_indent_cache(buffer: &Buffer) -> usize {
    let scopes = buffer.cached_indent_scopes();
    let mut checksum = scopes.iter().fold(0usize, |acc, scope| {
        acc ^ scope.id
            ^ scope.start_line
            ^ scope.end_line.unwrap_or(usize::MAX)
            ^ scope.indent_width
            ^ usize::from(scope.is_active())
    });

    for line_idx in 0..buffer.line_count() {
        if let Some(scope_ids) = buffer.cached_line_indent_scope_ids(line_idx) {
            checksum ^= scope_ids
                .iter()
                .fold(0usize, |acc, scope_id| acc ^ *scope_id);
        }
    }

    checksum
}

fn bench_indent_scope_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("indent_scope_index_build");

    for scenario in SCENARIOS {
        let text = scenario_text(scenario);
        group.bench_function(format!("{}/full_build", scenario.name), |b| {
            b.iter_batched(
                || Buffer::from_str(&text),
                |mut buffer| {
                    buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
                    black_box(checksum_indent_cache(&buffer))
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_indent_scope_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("indent_scope_index_clone");

    for scenario in SCENARIOS {
        let buffer = cached_buffer(scenario);
        group.bench_function(format!("{}/buffer_clone", scenario.name), |b| {
            b.iter(|| {
                let cloned = black_box(&buffer).clone();
                black_box(checksum_indent_cache(&cloned))
            })
        });
    }

    group.finish();
}

fn bench_indent_scope_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("indent_scope_index_lookup");

    for scenario in SCENARIOS {
        let buffer = cached_buffer(scenario);
        let line_count = buffer.line_count();
        group.bench_function(format!("{}/all_line_scope_ids", scenario.name), |b| {
            b.iter(|| {
                let mut checksum = 0usize;
                for line_idx in 0..line_count {
                    if let Some(scope_ids) =
                        buffer.cached_line_indent_scope_ids(black_box(line_idx))
                    {
                        checksum ^= scope_ids
                            .iter()
                            .fold(0usize, |acc, scope_id| acc ^ *scope_id);
                    }
                }
                black_box(checksum)
            })
        });
    }

    group.finish();
}

fn bench_indent_scope_invalidation(c: &mut Criterion) {
    let mut group = c.benchmark_group("indent_scope_index_invalidation");

    for scenario in SCENARIOS {
        let buffer = cached_buffer(scenario);
        let edit_line = scenario.line_count / 3;
        group.bench_function(format!("{}/insert_and_rebuild", scenario.name), |b| {
            b.iter_batched(
                || buffer.clone(),
                |mut buffer| {
                    buffer.insert_text(Cursor::new(edit_line, 0), "  inserted\n");
                    buffer.ensure_syntax_through(buffer.line_count().saturating_sub(1));
                    black_box(checksum_indent_cache(&buffer))
                },
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_indent_scope_build,
    bench_indent_scope_clone,
    bench_indent_scope_lookup,
    bench_indent_scope_invalidation
);
criterion_main!(benches);
