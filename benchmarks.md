# Summary

PieceTable showed a clear advantage on editor-shaped mutation workloads. It was substantially faster for mixed editing, linewise operations, and full-text materialization, while remaining clone-cheap and competitive on simple insert/remove paths. LineText still had an edge in some cursor-conversion and construction cases, but those differences were relatively small compared with the gains on editing-heavy workloads. Overall, the benchmark results support PieceTable as the better long-term storage backend for mutation performance, with the main remaining areas to watch being from_text and cursor-conversion overhead.

# Criterion Results (LineText vs PieceTable)

```
construct/line_text/from_text
                        time:   [16.764 µs 16.788 µs 16.810 µs]
                        change: [+7.7978% +8.1147% +8.4258%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 3 outliers among 100 measurements (3.00%)
  1 (1.00%) low mild
  2 (2.00%) high mild
construct/piece_table/from_text
                        time:   [16.311 µs 16.405 µs 16.500 µs]
                        change: [-2.1277% -1.5554% -0.9712%] (p = 0.00 < 0.05)
                        Change within noise threshold.

insert_char/line_text/insert_char
                        time:   [657.21 ns 669.71 ns 679.96 ns]
                        change: [-6.4901% -0.8839% +5.6329%] (p = 0.77 > 0.05)
                        No change in performance detected.
insert_char/piece_table/insert_char
                        time:   [528.12 ns 554.31 ns 576.89 ns]
                        change: [-0.4300% +7.1137% +15.522%] (p = 0.07 > 0.05)
                        No change in performance detected.

insert_multiline/line_text/insert_text
                        time:   [5.5498 µs 5.5734 µs 5.5920 µs]
                        change: [-1.9183% -0.6089% +0.7098%] (p = 0.37 > 0.05)
                        No change in performance detected.
insert_multiline/piece_table/insert_text
                        time:   [702.15 ns 722.57 ns 740.92 ns]
                        change: [-7.5544% -2.9482% +2.1514%] (p = 0.25 > 0.05)
                        No change in performance detected.

remove_range/line_text/remove
                        time:   [4.6398 µs 4.6643 µs 4.6833 µs]
                        change: [-2.4000% -0.6320% +1.1532%] (p = 0.48 > 0.05)
                        No change in performance detected.
remove_range/piece_table/remove
                        time:   [1.1501 µs 1.1844 µs 1.2132 µs]
                        change: [-6.9616% -1.4410% +4.0153%] (p = 0.61 > 0.05)
                        No change in performance detected.

read_line/line_text/line
                        time:   [29.759 ns 29.763 ns 29.766 ns]
                        change: [-0.1913% +0.1993% +0.5858%] (p = 0.32 > 0.05)
                        No change in performance detected.
Found 6 outliers among 100 measurements (6.00%)
  3 (3.00%) high mild
  3 (3.00%) high severe
read_line/piece_table/line
                        time:   [24.373 ns 24.398 ns 24.430 ns]
                        change: [+0.9302% +1.1694% +1.5721%] (p = 0.00 < 0.05)
                        Change within noise threshold.
Found 12 outliers among 100 measurements (12.00%)
  1 (1.00%) low mild
  7 (7.00%) high mild
  4 (4.00%) high severe

full_text/line_text/text
                        time:   [7.9403 µs 7.9510 µs 7.9657 µs]
                        change: [-0.3543% +0.4692% +1.3635%] (p = 0.27 > 0.05)
                        No change in performance detected.
full_text/piece_table/text
                        time:   [6.5059 µs 6.5213 µs 6.5403 µs]
                        change: [-4.6463% -4.3746% -4.1090%] (p = 0.00 < 0.05)
                        Performance has improved.

cursor_conversions/line_text/byte_offset_for_cursor                                                                                                                  time:   [17.464 µs 17.481 µs 17.501 µs]
                        change: [-8.3685% -7.2278% -6.0935%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 3 outliers among 100 measurements (3.00%)
  2 (2.00%) high mild
  1 (1.00%) high severe
cursor_conversions/piece_table/byte_offset_for_cursor
                        time:   [2.3756 µs 2.3784 µs 2.3826 µs]
                        change: [+4.9877% +5.3352% +5.7144%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 9 outliers among 100 measurements (9.00%)
  1 (1.00%) high mild
  8 (8.00%) high severe
cursor_conversions/line_text/cursor_for_byte_offset
                        time:   [18.947 µs 19.059 µs 19.171 µs]
                        change: [-1.4394% -1.1544% -0.9245%] (p = 0.00 < 0.05)
                        Change within noise threshold.
Found 24 outliers among 100 measurements (24.00%)
  9 (9.00%) low severe
  2 (2.00%) low mild
  3 (3.00%) high mild
  10 (10.00%) high severe
cursor_conversions/piece_table/cursor_for_byte_offset                                                                                                                time:   [2.5560 µs 2.5573 µs 2.5587 µs]
                        change: [+4.9941% +5.1050% +5.2196%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 1 outliers among 100 measurements (1.00%)
  1 (1.00%) high severe

clone/line_text/clone   time:   [8.4187 ns 8.4234 ns 8.4328 ns]
                        change: [+0.4173% +0.5190% +0.6333%] (p = 0.00 < 0.05)
                        Change within noise threshold.
Found 9 outliers among 100 measurements (9.00%)
  1 (1.00%) low mild
  1 (1.00%) high mild
  7 (7.00%) high severe
clone/piece_table/clone time:   [3.3286 ns 3.3347 ns 3.3405 ns]
                        change: [+2.4793% +2.6586% +2.8476%] (p = 0.00 < 0.05)
                        Performance has regressed.
Found 20 outliers among 100 measurements (20.00%)
  1 (1.00%) low mild
  17 (17.00%) high mild
  2 (2.00%) high severe

linewise_ops/line_text/join_lines
                        time:   [4.1180 µs 4.1379 µs 4.1552 µs]
                        change: [+0.0274% +1.0106% +2.0781%] (p = 0.06 > 0.05)
                        No change in performance detected.
linewise_ops/piece_table/join_lines
                        time:   [638.33 ns 671.48 ns 708.00 ns]
                        change: [-4.5306% +1.7372% +8.3224%] (p = 0.58 > 0.05)
                        No change in performance detected.
Found 2 outliers among 100 measurements (2.00%)
  2 (2.00%) high mild
linewise_ops/line_text/delete_lines
                        time:   [3.8858 µs 3.9003 µs 3.9138 µs]
                        change: [-1.3318% -0.4308% +0.5304%] (p = 0.37 > 0.05)
                        No change in performance detected.
linewise_ops/piece_table/delete_lines
                        time:   [693.35 ns 723.22 ns 749.99 ns]
                        change: [-4.0349% +2.7720% +9.9569%] (p = 0.44 > 0.05)
                        No change in performance detected.
linewise_ops/line_text/change_lines
                        time:   [3.9123 µs 3.9250 µs 3.9362 µs]
                        change: [-1.3842% -0.5851% +0.2207%] (p = 0.15 > 0.05)
                        No change in performance detected.
linewise_ops/piece_table/change_lines
                        time:   [790.69 ns 813.13 ns 832.65 ns]
                        change: [-6.5980% -1.2323% +4.5902%] (p = 0.68 > 0.05)
                        No change in performance detected.
linewise_ops/line_text/paste_linewise
                        time:   [4.0815 µs 4.0944 µs 4.1073 µs]
                        change: [-1.2281% -0.3906% +0.4497%] (p = 0.37 > 0.05)
                        No change in performance detected.
linewise_ops/piece_table/paste_linewise
                        time:   [644.73 ns 667.51 ns 688.07 ns]
                        change: [-5.5305% -0.9039% +3.4436%] (p = 0.69 > 0.05)
                        No change in performance detected.

Benchmarking mixed_edits/line_text/mixed: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 7.7s, enable flat sampling, or reduce sample count to 50.
mixed_edits/line_text/mixed
                        time:   [1.2903 ms 1.2913 ms 1.2924 ms]
                        change: [-1.5873% -1.4014% -1.2397%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 2 outliers among 100 measurements (2.00%)
  1 (1.00%) high mild
  1 (1.00%) high severe
mixed_edits/piece_table/mixed
                        time:   [195.52 µs 195.71 µs 195.90 µs]
                        change: [+0.6569% +0.8718% +1.0940%] (p = 0.00 < 0.05)
                        Change within noise threshold.
Found 4 outliers among 100 measurements (4.00%)
  3 (3.00%) low mild
  1 (1.00%) high mild

typing_run/line_text/100_chars
                        time:   [34.596 µs 35.342 µs 36.310 µs]
                        change: [-0.8903% +0.1564% +1.5726%] (p = 0.81 > 0.05)
                        No change in performance detected.
Found 3 outliers among 100 measurements (3.00%)
  3 (3.00%) high severe
typing_run/piece_table/100_chars
                        time:   [20.060 µs 20.680 µs 21.427 µs]
                        change: [-9.0833% -0.8037% +8.0780%] (p = 0.86 > 0.05)
                        No change in performance detected.
Found 21 outliers among 100 measurements (21.00%)
  5 (5.00%) high mild
  16 (16.00%) high severe
```
