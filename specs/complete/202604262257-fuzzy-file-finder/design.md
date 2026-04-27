# Fuzzy File Finder - Technical Design

## Architecture Overview
The fuzzy finder is a modal overlay widget layered above the editor layout. It reuses the existing widget and intent pipeline, with `Layout` owning the picker as an overlay alongside other transient UI surfaces such as the command line and confirmation box.

The implementation is split into three parts:
- a generic picker engine that owns query text, highlighted index, result list, and lifecycle state
- a concrete file-picker source that produces file results and selection behavior
- a background search worker that walks the filesystem and streams matching results back in chunks

Data flow is:
`F1` -> `Command::OpenFilePicker` -> `Layout` opens picker overlay -> picker accepts keystrokes -> query change increments search generation -> file search worker scans from the current working directory using a gitignore-aware recursive iterator -> chunked result updates stream back to the picker -> picker renders incremental results -> `Enter` / `Ctrl-Y` emit a file-open intent

The design keeps the picker generic over result type by separating the picker state machine from the result source. The generic picker does not know how results are discovered or selected; it only knows how to display, filter highlight state, and forward selection to the source-provided action.

## Interface Design

| Interface | Input | Output | Description |
|-----------|-------|--------|-------------|
| `Command::OpenFilePicker` | none | picker overlay opened | Opens the file picker overlay |
| `Layout::open_file_picker()` | none | `()` | Creates and installs the active file picker overlay |
| `PickerWidget<T>::handle_ui_event()` | `UiEvent`, `UiContext` | `UiEventResult` | Handles query edits, navigation, cancel, and selection |
| `PickerWidget<T>::render_widget()` | `Screen`, `UiRect`, `UiContext` | `()` | Renders search bar and results list |
| `PickerSource<T>::start_search()` | `query`, `generation`, `sender` | `()` | Starts asynchronous result streaming for the current query |
| `PickerSource<T>::select()` | highlighted result | `Intent` | Converts a chosen result into a concrete action |
| `FilePickerSource::start_search()` | query text, generation, sender | `()` | Streams matching file entries from the current working directory |

## Data Models

| Field | Type | Constraints | Description |
|-------|------|-------------|-------------|
| `PickerState.query` | `String` | always valid UTF-8 | Current search text |
| `PickerState.results` | `Vec<T>` | ordered by arrival / ranking | Live result list shown in the overlay |
| `PickerState.highlighted` | `Option<usize>` | valid index when present | Currently selected row |
| `PickerState.generation` | `u64` | monotonically increasing | Discards stale worker output |
| `PickerSearchEvent::PickerSearchStarted` | `{ generation, query }` | first event for a search | Signals that a new search generation began |
| `PickerSearchEvent::PickerChunk` | `{ generation, chunk }` | non-empty chunk | Incremental result batch |
| `PickerSearchEvent::PickerSearchStale` | `{ generation }` | terminal stale event | Signals that a search result stream is stale |
| `PickerSearchEvent::PickerSearchComplete` | `{ generation }` | terminal event | Marks the current search as complete |
| `FilePickerItem.path` | `PathBuf` | absolute or cwd-relative accepted | File selected from the picker |
| `FilePickerItem.label` | `String` | non-empty display text | Rendered result label |

Schema changes:
- No persisted schema changes are required.
- The new picker state is runtime-only.

## Key Components

### PickerWidget<T>
**Responsibilities:**
- Own the shared search bar and results UI
- Track query text, highlighted row, and active generation
- Consume streamed picker updates and ignore stale generations
- Emit selection intents and cancel/close behavior

**Public API:**
- `new(source: impl PickerSource<T>)`
- `handle_ui_event(event: &UiEvent, ctx: &mut UiContext) -> UiEventResult`
- `render_widget(screen: &mut Screen, rect: UiRect, ctx: &UiContext)`
- `is_open() -> bool`

**Notes:**
- The widget stays generic and does not hard-code file behavior.
- `Esc` and `Ctrl-C` close the overlay without emitting a selection.

### PickerSource<T>
**Responsibilities:**
- Start asynchronous discovery for the current query
- Define how a selected result becomes an intent

**Public API:**
- `start_search(query: &str, generation: u64, sender: Sender<PickerUpdate<T>>)`
- `select(item: &T) -> Intent`

### FilePickerSource
**Responsibilities:**
- Search files under the current working directory
- Apply case-insensitive matching
- Exclude directories from the result set
- Open the selected file in a new tab or focus an existing tab

**Public API:**
- `new(cwd: PathBuf)`
- `start_search(query: &str, generation: u64, sender: Sender<PickerUpdate<FilePickerItem>>)`
- `select(item: &FilePickerItem) -> Intent`

### PickerSearchWorker
**Responsibilities:**
- Walk the filesystem on a background thread
- Batch matches into chunks
- Stop producing results once the generation is stale or the picker closes
- Emit the picker search event stream in order

**Algorithm sketch:**
1. Capture the query and generation at submission time.
2. Traverse from the current working directory with a gitignore-aware recursive iterator.
3. Filter out non-file entries.
4. Compare candidate paths case-insensitively against the query.
5. Buffer matches into chunks and send each chunk immediately.
6. Send `PickerSearchStarted` before traversal work begins.
7. Send `PickerChunk` events as chunks are produced.
8. Send `PickerSearchStale` if the generation becomes invalid before completion.
9. Send `PickerSearchComplete` when traversal completes.

## User Interaction
### Invocation Patterns
- `F1` opens the file picker overlay.
- Typing while the picker is open edits the query and restarts the search.
- `Up` / `Down` move the highlighted result.
- `Ctrl-N` / `Ctrl-P` move the highlighted result down and up.
- `Enter` and `Ctrl-Y` select the highlighted result.
- `Esc` and `Ctrl-C` close the picker without selecting.

### Flows
1. User presses `F1`.
2. Layout opens a file picker overlay and clears previous picker state.
3. User types a query.
4. Picker clears displayed results and starts a new generation.
5. Background worker streams matching files in chunks.
6. Picker appends chunks as they arrive and keeps the highlight valid.
7. User selects a result.
8. File picker resolves the selection into an open-or-focus file action.

### Error and Recovery Paths
- If a search finishes with no matches, the picker remains open and shows an empty state.
- If a stale worker chunk arrives after a new query, it is ignored by generation check.
- If the worker detects staleness during traversal, it emits `PickerSearchStale` and stops.
- If file opening fails on selection, the picker closes only after the failure intent is handled by the editor.

## External Dependencies
| Dependency | Purpose | Version/Notes |
|------------|---------|---------------|
| `ignore` | Recursive filesystem traversal with gitignore support | Add as new dependency |
| `std::sync::mpsc` | Streaming chunk delivery | Existing stdlib channel |
| `std::thread` | Background search worker | Existing stdlib thread |
| current working directory | Search root | Provided by process/runtime |

## Error Handling
| Error Code | Condition | Error Data | Recovery |
|------------|-----------|------------|----------|
| `PICKER_SEARCH_EMPTY` | No matches found | `{ query }` | Keep picker open, show empty state |
| `PICKER_SEARCH_STALE` | Chunk belongs to old generation | `{ generation }` | Drop chunk silently |
| `PICKER_SEARCH_CANCELLED` | Picker closed before search completed | `{ generation }` | Stop applying updates |
| `PICKER_OPEN_FAILED` | File action cannot open path | `{ path, message }` | Surface an editor notification |

Logging requirements:
- Search start/finish events should be debug-level.
- Search failures should be warn-level.
- Selection failures should be reported through the normal notification path.

## Security
- File paths are treated as data, not commands.
- Search input is never executed.
- The picker must not reveal file contents, only paths/labels.
- Background search should stay within the current working directory tree.

## Configuration
No new user-facing configuration is required for phase 1.

Fixed behavior:
- case-insensitive matching
- files only, no directories
- current working directory as search root
- chunked async updates

## Component Interactions
```text
Keyboard input -> Layout -> PickerWidget<T>
PickerWidget<T> -> PickerSource<T>::start_search
PickerSearchWorker -> mpsc channel -> PickerWidget<T>
PickerWidget<T> -> PickerSource<T>::select -> Intent
Intent -> Layout dispatch -> open file / focus existing tab
```

Picker routing is overlay-first, matching the existing command-line and confirmation-box behavior. When the picker is open, it captures search keystrokes before they reach the editor buffer.

## Platform Considerations
| Platform | Consideration | Approach |
|----------|---------------|----------|
| macOS | Path display and cwd resolution | Use standard `PathBuf` / process cwd |
| Linux | Large directory trees | Stream results in chunks to keep UI responsive |
| Windows | Path separator differences | Preserve native path display while matching case-insensitively |

## Trade-offs
**Decision**: Use a generic picker engine plus concrete sources instead of a single file-only widget.

**Reasoning**:
- keeps file picker logic isolated from shared UI behavior
- makes future result types a source-only change
- preserves the existing widget and intent model

**Impact**:
- slightly more initial type structure
- requires one extra layer of abstraction for result streaming

**Decision**: Use a dedicated background search worker with generation tokens.

**Reasoning**:
- chunked streaming is easier to express with a worker/channel pair
- generation tokens make stale query results cheap to discard
- avoids blocking the main event loop on traversal

**Impact**:
- adds one background thread per active picker session
- requires careful cancellation and stale-result handling

## Risks and Mitigations
| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-------------|
| Large directory scans flood the UI | Medium | High | Batch results into chunks and render incrementally |
| Stale search output races with new queries | Medium | High | Use generation tokens and ignore mismatched chunks |
| Picker highlight becomes invalid after result replacement | Medium | Medium | Clamp highlight to current result count after each update |
| File-open action conflicts with already-open tabs | Low | Medium | Reuse existing open-or-focus tab behavior |
| Cwd lookup fails when picker opens | Low | Medium | Fall back to an empty result set and emit notification |
