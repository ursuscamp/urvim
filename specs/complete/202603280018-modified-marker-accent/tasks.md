# BUG-202603280018: Modified marker accent bug - Implementation Tasks

## Overview

Total: 4 tasks
Add a foreground-only style composition helper for accent markers and use it when rendering the modified-buffer indicator in the tab bar and status bar.

## Implementation Tasks

- [x] **1.** Add a foreground-only `Style::accent` compositor
  - [x] **1.1** Implement `accent(self, other: Style)` in `src/terminal/style.rs` so it applies foreground attributes and decorations but never copies background (depends on: 1.2)
  - [x] **1.2** Add doc comments for the new public method and keep the method ordering aligned with the existing `Style` API

- [x] **2.** Switch modified marker rendering to `accent`
  - [x] **2.1** Update the tab bar marker style composition in `src/tab_group.rs` to use `accent` instead of `overlay`
  - [x] **2.2** Update the status bar marker style composition in `src/status_bar.rs` to use `accent` instead of `overlay`

- [x] **3.** Add regression tests for accent composition and marker rendering
  - [x] **3.1** Add a `Style::accent` unit test that preserves the base background while applying foreground attributes
  - [x] **3.2** Update or add tab/status bar tests to confirm modified markers keep the surrounding background

- [x] **4.** Run project checks
  - [x] **4.1** Run `cargo check`
  - [x] **4.2** Run the affected unit tests for style, tab group, and status bar rendering

## Completion Summary

| Phase | Tasks | Done |
| --- | --- | --- |
| Style compositor | 2 | 2 |
| Marker rendering | 2 | 2 |
| Testing | 2 | 2 |
| Total | 6 | 6 |
