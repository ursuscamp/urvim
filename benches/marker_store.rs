use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use std::hint::black_box as std_black_box;
use urvim::buffer::{Cursor, DeleteShape, Gravity, InsertShape, MarkerId, MarkerStore};

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    line_count: usize,
    marker_count: usize,
    cluster_start: Option<usize>,
}

const SCENARIOS: [Scenario; 6] = [
    Scenario {
        name: "empty_10k",
        line_count: 10_000,
        marker_count: 0,
        cluster_start: None,
    },
    Scenario {
        name: "sparse_10k",
        line_count: 10_000,
        marker_count: 100,
        cluster_start: None,
    },
    Scenario {
        name: "clustered_10k",
        line_count: 10_000,
        marker_count: 1_000,
        cluster_start: Some(4_000),
    },
    Scenario {
        name: "large_sparse_50k",
        line_count: 50_000,
        marker_count: 500,
        cluster_start: None,
    },
    Scenario {
        name: "dense_10k",
        line_count: 10_000,
        marker_count: 10_000,
        cluster_start: None,
    },
    Scenario {
        name: "dense_50k",
        line_count: 50_000,
        marker_count: 50_000,
        cluster_start: None,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadKind {
    Ghost,
    Inlay,
}

fn populate_store(scenario: Scenario) -> (MarkerStore<PayloadKind>, Vec<MarkerId>) {
    let mut store = MarkerStore::with_line_count(scenario.line_count);
    let mut ids = Vec::with_capacity(scenario.marker_count);
    for idx in 0..scenario.marker_count {
        let line = marker_line(idx, scenario);
        let payload = if idx % 3 == 0 {
            PayloadKind::Inlay
        } else {
            PayloadKind::Ghost
        };
        ids.push(store.insert_point(Cursor::new(line, idx % 80), Gravity::Right, payload));
    }
    (store, ids)
}

fn marker_line(idx: usize, scenario: Scenario) -> usize {
    if scenario.marker_count == 0 {
        return 0;
    }
    if let Some(start) = scenario.cluster_start {
        return (start + idx % 120).min(scenario.line_count.saturating_sub(1));
    }
    (idx * scenario.line_count / scenario.marker_count).min(scenario.line_count.saturating_sub(1))
}

fn bench_marker_store(c: &mut Criterion) {
    bench_construct(c);
    bench_lookup_visible(c);
    bench_lookup_all_lines(c);
    bench_clone(c);
    bench_clone_then_mutate(c);
    bench_get_remove(c);
    bench_shift_insert_single_line(c);
    bench_shift_insert_multi_line(c);
    bench_shift_delete_single_line(c);
    bench_shift_delete_multi_line(c);
    bench_insert_lines(c);
    bench_delete_lines(c);
    bench_clear_inlay_hints_for_lines(c);
}

fn bench_construct(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_construct");
    for scenario in SCENARIOS {
        group.bench_function(scenario.name, |b| {
            b.iter(|| {
                std_black_box(MarkerStore::<PayloadKind>::with_line_count(
                    scenario.line_count,
                ))
            })
        });
    }
    group.finish();
}

fn bench_lookup_visible(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_lookup_visible");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        let line = scenario.line_count / 2;
        group.bench_function(scenario.name, move |b| {
            b.iter(|| black_box(store.markers_for_line(line)))
        });
    }
    group.finish();
}

fn bench_lookup_all_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_lookup_all_lines");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter(|| {
                let total: usize = (0..scenario.line_count)
                    .filter_map(|line| store.markers_for_line(line))
                    .map(|markers| markers.len())
                    .sum();
                black_box(total)
            })
        });
    }
    group.finish();
}

fn bench_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_clone");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| b.iter(|| black_box(store.clone())));
    }
    group.finish();
}

fn bench_clone_then_mutate(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_clone_then_mutate");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.shift_insert(InsertShape {
                        at: Cursor::new(scenario.line_count / 2, 10),
                        line_delta: 1,
                        tail_col: 2,
                    });
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_get_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_get_remove");
    for scenario in SCENARIOS {
        let (store, ids) = populate_store(scenario);
        let target = ids
            .get(ids.len().saturating_sub(1) / 2)
            .copied()
            .unwrap_or(0);

        group.bench_function(format!("{}/get", scenario.name), |b| {
            b.iter(|| black_box(store.get(target)))
        });

        group.bench_function(format!("{}/remove", scenario.name), |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    black_box(cloned.remove(target));
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_shift_insert_single_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_shift_insert_single_line");
    let edit = InsertShape {
        at: Cursor::new(5_000, 12),
        line_delta: 0,
        tail_col: 6,
    };
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.shift_insert(edit);
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_shift_insert_multi_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_shift_insert_multi_line");
    let edit = InsertShape {
        at: Cursor::new(5_000, 12),
        line_delta: 4,
        tail_col: 6,
    };
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.shift_insert(edit);
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_shift_delete_single_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_shift_delete_single_line");
    let edit = DeleteShape {
        start: Cursor::new(5_000, 12),
        end: Cursor::new(5_000, 18),
    };
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.shift_delete(edit);
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_shift_delete_multi_line(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_shift_delete_multi_line");
    let edit = DeleteShape {
        start: Cursor::new(5_000, 12),
        end: Cursor::new(5_012, 18),
    };
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.shift_delete(edit);
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_insert_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_insert_lines");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.insert_lines(scenario.line_count / 2, 3);
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_delete_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_delete_lines");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.delete_lines(scenario.line_count / 2, 3);
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_clear_inlay_hints_for_lines(c: &mut Criterion) {
    let mut group = c.benchmark_group("marker_store_clear_inlay_hints_for_lines");
    for scenario in SCENARIOS {
        let (store, _) = populate_store(scenario);
        group.bench_function(scenario.name, move |b| {
            b.iter_batched(
                || store.clone(),
                |mut cloned| {
                    cloned.retain_in_line_range(0, scenario.line_count / 2, |marker| {
                        matches!(marker.payload, PayloadKind::Ghost)
                    });
                    black_box(cloned);
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_marker_store);
criterion_main!(benches);
