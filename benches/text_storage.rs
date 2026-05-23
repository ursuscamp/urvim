use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use urvim::buffer::{Cursor, PieceTable, TextRef, TextSnapshot, TextStorage};

fn sample_text() -> String {
    (0..500)
        .map(|idx| format!("line {idx}: The quick brown fox jumps over the lazy dog {idx}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn large_sample_text() -> String {
    (0..5_000)
        .map(|idx| format!("line {idx}: The quick brown fox jumps over the lazy dog {idx}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn bench_construct(c: &mut Criterion) {
    let text = sample_text();

    c.bench_function("piece_table/from_text", |b| {
        b.iter(|| PieceTable::from_text(black_box(&text)))
    });
}

fn bench_insert_char(c: &mut Criterion) {
    let text = sample_text();
    let cursor = Cursor::new(250, 12);

    c.bench_function("piece_table/insert_char", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage
                    .insert_char(black_box(cursor), black_box('x'))
                    .expect("insert");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_insert_multiline(c: &mut Criterion) {
    let text = sample_text();
    let cursor = Cursor::new(250, 12);
    let inserted = "alpha\nbeta\ngamma";

    c.bench_function("piece_table/insert_text", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage
                    .insert_text(black_box(cursor), black_box(inserted))
                    .expect("insert");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_remove_range(c: &mut Criterion) {
    let text = sample_text();
    let start = Cursor::new(200, 5);
    let end = Cursor::new(260, 8);

    c.bench_function("piece_table/remove", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage
                    .remove(black_box(start), black_box(end))
                    .expect("remove");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_read_line(c: &mut Criterion) {
    let text = sample_text();
    let line = 250;
    let piece_table = PieceTable::from_text(&text);

    c.bench_function("piece_table/line", |b| {
        b.iter(|| black_box(piece_table.line(line).expect("line")).to_text())
    });
}

fn bench_full_text(c: &mut Criterion) {
    let text = sample_text();
    let piece_table = PieceTable::from_text(&text);

    c.bench_function("piece_table/text", |b| {
        b.iter(|| black_box(piece_table.text().to_text()))
    });
}

fn bench_cursor_conversions(c: &mut Criterion) {
    let text = large_sample_text();
    let piece_table = PieceTable::from_text(&text);
    let cursor = Cursor::new(2_500, 12);
    let offset = piece_table
        .byte_offset_for_cursor(cursor)
        .expect("cursor should be valid");

    let mut group = c.benchmark_group("cursor_conversions");
    group.bench_function("piece_table/byte_offset_for_cursor", |b| {
        b.iter(|| black_box(piece_table.byte_offset_for_cursor(black_box(cursor))))
    });
    group.bench_function("piece_table/cursor_for_byte_offset", |b| {
        b.iter(|| black_box(piece_table.cursor_for_byte_offset(black_box(offset))))
    });
    group.finish();
}

fn bench_clone(c: &mut Criterion) {
    let text = large_sample_text();
    let piece_table = PieceTable::from_text(&text);

    c.bench_function("piece_table/clone", |b| {
        b.iter(|| black_box(piece_table.clone()))
    });
}

fn bench_linewise_ops(c: &mut Criterion) {
    let text = sample_text();

    let mut group = c.benchmark_group("linewise_ops");
    group.bench_function("piece_table/join_lines", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage.join_lines(100, 4, true).expect("join");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("piece_table/delete_lines", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage.delete_lines(100, 20).expect("delete");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("piece_table/change_lines", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage.change_lines(100, 20).expect("change");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
    group.bench_function("piece_table/paste_linewise", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                storage
                    .paste_linewise(100, ["alpha", "beta", "gamma"], true)
                    .expect("paste");
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_mixed_edits(c: &mut Criterion) {
    let text = large_sample_text();

    fn run_mixed_edits(mut storage: PieceTable) -> PieceTable {
        let line_idx = 1_000;
        for step in 0..100 {
            let line_len = storage.line(line_idx).map(|line| line.len()).unwrap_or(0);
            match step % 4 {
                0 => {
                    let cursor = Cursor::new(line_idx, line_len.min(8));
                    storage.insert_char(cursor, 'x').expect("insert");
                }
                1 => {
                    let cursor = Cursor::new(line_idx, line_len.min(4));
                    storage.insert_char(cursor, '\n').expect("newline");
                }
                2 => {
                    if line_len > 0 {
                        let start_col = line_len.saturating_sub(1);
                        let start = Cursor::new(line_idx, start_col);
                        let end = Cursor::new(line_idx, start_col + 1);
                        storage.remove(start, end).expect("remove");
                    } else {
                        storage
                            .insert_char(Cursor::new(line_idx, 0), 'z')
                            .expect("insert fallback");
                    }
                }
                _ => {
                    let cursor = Cursor::new(line_idx, line_len.min(6));
                    storage.insert_text(cursor, "ab\ncd").expect("insert text");
                }
            }
        }
        storage
    }

    c.bench_function("piece_table/mixed", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |storage| black_box(run_mixed_edits(storage)),
            BatchSize::SmallInput,
        )
    });
}

fn bench_typing_run(c: &mut Criterion) {
    let text = sample_text();
    let cursor = Cursor::new(250, 12);

    c.bench_function("piece_table/100_chars", |b| {
        b.iter_batched(
            || PieceTable::from_text(&text),
            |mut storage| {
                let mut cursor = cursor;
                for _ in 0..100 {
                    storage.insert_char(cursor, 'x').expect("insert");
                    cursor.col += 1;
                }
                black_box(storage)
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_construct,
    bench_insert_char,
    bench_insert_multiline,
    bench_remove_range,
    bench_read_line,
    bench_full_text,
    bench_cursor_conversions,
    bench_clone,
    bench_linewise_ops,
    bench_mixed_edits,
    bench_typing_run
);
criterion_main!(benches);
