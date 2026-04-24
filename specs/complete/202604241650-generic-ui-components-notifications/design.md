# Generic UI Components and Notification Infrastructure - Technical Design

## Architecture Overview
This design evolves urvim from a window-centric UI pipeline to a component-capable pipeline while retaining current behavior.

Current flow is primarily:
- terminal event -> active window mode key handling
- resolved `Action` -> `Layout` -> `WindowGroup` -> `Window`
- render path hardcodes pane content, tab bar, and footer status bar

Target phase-1 flow introduces:
- expanded `Widget` contract for broader UI responsibilities
- root-managed layered rendering/routing (base content + overlays)
- non-window components integrated into root flow (notification banner in phase 1)
- dual-write notification macros to logging + in-memory notification queue with sequential display
- a unified dispatch pipeline that processes both editor actions and UI orchestration commands through one intent queue

Phase 1 preserves existing action semantics and editing behavior. `Action` remains the editing-semantic model, `Command` remains the UI/app orchestration model, and both are carried through a unified intent-dispatch path.

## Interface Design

### Expanded Widget Contract
`src/widget.rs` will evolve from action-only handling to include UI-lifecycle behavior with default methods for backward compatibility.

Proposed interface shape (illustrative):
- `process_action(&mut self, action: &Action) -> ActionResult` (existing)
- `handle_ui_event(&mut self, event: &UiEvent, ctx: &mut UiContext) -> UiEventResult` (new)
- `layout(&mut self, constraints: UiConstraints) -> Size` (new)
- `render_widget(&mut self, screen: &mut Screen, rect: UiRect, ctx: &UiContext)` (new)
- `focus_policy(&self) -> FocusPolicy` (new, default passive)

Design constraints:
- `process_action` is transitional-only and exists to keep migration incremental.
- Existing callers that only use `process_action` must continue to compile during migration.
- New methods provide defaults where possible to minimize initial churn.
- By end of this implementation, primary dispatch should route through unified intents; remaining `process_action` usage should be intentionally limited and tracked for removal.

### UI Event and Intent Interfaces
Introduce UI-facing types (new module, e.g. `src/ui/mod.rs`):
- `UiEvent`: internal UI-routing input (derived from terminal events plus UI lifecycle signals)
- `UiEventResult`: handled/not-handled plus optional intent emissions
- `Command`: orchestration commands (e.g., show notification, set focus target, open/close overlay)
- `Intent`: unified dispatch envelope, with variants such as `Intent::Action(Action)` and `Intent::Command(Command)`

Protocol:
- Components consume `UiEvent` and may emit one or more `Intent` values.
- Root dispatcher consumes one intent stream and applies side effects to shared UI/app state.
- `Action` and `Command` stay distinct domain types, but dispatch handling is unified.

### Notification Macros
Add globally available macros:
- `notify!(level, ...)`
- `notify_info!(...)`
- `notify_warn!(...)`
- `notify_error!(...)`

Behavior contract:
- Format message using standard Rust formatting arguments.
- Write to `debug.log` through existing logging path at matching level.
- Enqueue notification into UI notification queue for sequential display.

### Notification Integration Points
Phase-1 call sites:
- Save success path emits `info` notification.
- Curated user-impacting failure paths emit `error` notification.

## Data Models

### NotificationLevel
New enum:
- `Info`
- `Warn`
- `Error`

Constraints:
- Used for style resolution and log level mapping.

### NotificationMessage
New struct:
- `level: NotificationLevel`
- `text: String`
- `created_at: Instant`
- `expires_at: Instant`

Constraints:
- `expires_at = created_at + 3s` for phase 1.
- Empty messages are ignored at enqueue boundary.

### NotificationState
New runtime state container:
- `active: Option<NotificationMessage>`
- `pending: VecDeque<NotificationMessage>`

Constraints:
- Queue policy is FIFO in enqueue order.
- Queue is unbounded in this phase.
- The active message expires on tick/render checks; when expired, next queued message becomes active.
- Message TTL is adaptive: 3 seconds when no backlog exists; 1 second when one or more pending messages exist.

### UI Layer Model
Root-layer state in `Layout` (or a dedicated UI root state object) includes:
- base layer (existing split tree content)
- overlay entries (phase-1 includes notification banner)

Constraints:
- Overlay render order is deterministic.
- Phase 1 supports at least one overlay component.

## Key Components

### 1) Root UI Dispatcher (Layout-side)
Responsibilities:
- Route UI events to appropriate component targets.
- Consume a unified intent queue containing both `Action` and `Command` payloads.
- Maintain base + overlay rendering order.
- Own/coordinate shared UI context access for child components.

Public API impacts:
- Existing `Layout::process_action` remains only as a compatibility shim during migration, backed internally by intent-dispatch semantics.
- Additional UI event handling entrypoint added and integrated in main loop.
- Follow-up cleanup after migration will remove direct `process_action` entrypoints once call sites have moved to unified intent dispatch.

Dependencies:
- `Screen`, `Terminal` event loop integration, existing pane geometry/render code, intent types.

### 2) Notification Queue Service
Responsibilities:
- Store active and pending queued notifications.
- Advance queue when active entry expires.
- Provide read access for notification banner render.

Public API impacts:
- `enqueue(level, text, now)`
- `active(now)`
- `prune_and_advance(now)`

Dependencies:
- Time source (`Instant`), global/runtime state integration.

### 3) Notification Banner Widget
Responsibilities:
- Read active queued notification and render top-right.
- Apply level-based theme styles.
- Render non-focus-stealing, non-interactive output.

Public API impacts:
- Stateless render entry or lightweight state holder (preferred stateless with service dependency).

Dependencies:
- Theme resolution, `Screen` text measuring/clipping helpers.

### 4) Notification Macros + Bridge
Responsibilities:
- unify log + queue writes in one call site primitive.
- avoid ad hoc duplicated log and UI code.

Public API impacts:
- new macro exports and backing functions.

Dependencies:
- logging facade + notification queue service.

## User Interaction
- User saves a file successfully:
  - save executes as today
  - info notification appears top-right
  - message auto-disappears after approximately 3 seconds when no backlog exists
- User triggers user-impacting failure (e.g. save failure):
  - error notification appears top-right
  - style indicates severity
  - message auto-disappears based on adaptive TTL policy
- User triggers a burst of notifications:
  - notifications queue in FIFO order
  - banner shows one at a time
  - each message displays for approximately 1 second while backlog remains
- Existing editing and pane interactions remain unchanged.
- No completion popup behavior appears in this phase.

## External Dependencies
No new third-party dependencies are required.

Existing dependencies reused:
- `tracing` / logger initialization for `debug.log`
- terminal/screen rendering stack
- theme/style resolution stack

## Error Handling
- Macro formatting failure is not expected; normal Rust formatting rules apply.
- If notification enqueue fails due to unavailable runtime state during startup/teardown:
  - log write still proceeds
  - enqueue failure is non-fatal and should not panic
- Because the phase-1 queue is unbounded, sustained notification floods may increase memory use; this is accepted for phase 1 and can be bounded in a later phase if needed.
- Expired notification handling is best-effort and idempotent.
- Render clipping at small terminal sizes must avoid panics/out-of-bounds writes.

Recovery strategies:
- fall back to log-only behavior when queue service is unavailable
- skip banner render if no active message or zero-size target area

## Security
- Notification text is local process output; no external command execution.
- No secrets should be introduced into notification payloads beyond existing log practices.
- Messages are rendered as plain text; no escape-sequence injection should be introduced by notification rendering helpers.

## Configuration
Phase 1 introduces no user-facing config knobs.

Fixed behavior:
- top-right placement
- FIFO sequential queueing
- adaptive TTL: 3 seconds when no backlog exists, 1 second while backlog exists

Theme integration:
- notification level tags/styles resolved from theme with safe defaults when absent.

## Component Interactions
1. Terminal event loop receives `Event`.
2. Event adapter translates terminal input/lifecycle into `UiEvent`.
3. Root UI dispatcher routes `UiEvent` to components.
4. Components emit unified `Intent` values (`Intent::Action` and/or `Intent::Command`).
5. Root dispatcher consumes intents from a single queue and applies effects.
6. Save/error flows use notify macros; macros write log entries and enqueue notifications.
7. On frame render, notification banner reads active queued notification and renders at top-right.
8. Tick/render prune advances the queue when active notifications expire.

Interaction with existing paths:
- Editing semantics remain represented by `Action` and are executed through the unified intent dispatcher.
- `process_action` remains temporarily as a compatibility shim, not a long-term parallel architecture.
- New UI routing hooks expand capability rather than replacing editor behavior in phase 1.

## Platform Considerations
- Terminal width constraints require clipping/truncation for long notification text.
- Unicode width handling should reuse existing width utilities for right alignment correctness.
- Behavior should be consistent across ANSI terminals already supported by urvim.
- Notification timing relies on monotonic `Instant` and should be robust to frame/tick cadence variance.
