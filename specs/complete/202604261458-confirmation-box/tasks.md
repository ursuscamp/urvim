# Confirmation Box - Implementation Tasks

## Overview
Implement a reusable confirmation box widget and wire it into the editor's intent flow so quit attempts can first check for modified buffers, then either quit immediately or ask the user to confirm quitting. The component should be keyboard-driven, overlay-based, and reusable for future yes/no prompts.

## Backend
- [x] **1.** Add a reusable confirmation box widget/state type that stores the query text, the caller-supplied positive intent, and open/closed state.
  - [x] **1.1** Add constructors and accessors for the query and open state. (depends on: 1)
  - [x] **1.2** Implement key handling for `Y`, `N`, `Enter`, and `Esc`, returning the stored positive intent on confirm and no intent on cancel. (depends on: 1)
  - [x] **1.3** Add rendering for a short bordered prompt with a visible yes/no cue. (depends on: 1)
- [x] **2.** Add a `TryQuit`-style command or intent path that represents an attempted quit rather than an immediate exit.
  - [x] **2.1** Extend the UI command or action model with a quit-attempt variant that can be intercepted by the dispatcher. (depends on: 1)
  - [x] **2.2** Update the existing `<C-q>` binding to emit the quit-attempt path instead of direct quit. (depends on: 2.1)
- [x] **3.** Update the layout/dispatcher flow so quit attempts check modified buffers before deciding whether to quit immediately or open the confirmation box.
  - [x] **3.1** Detect modified buffers at the quit-attempt entry point and dispatch a direct quit when none are modified. (depends on: 2)
  - [x] **3.2** Open the confirmation box with `Quit` as the positive intent when modified buffers are present. (depends on: 1, 3.1)
  - [x] **3.3** Route confirmation-box events before base editor events while the prompt is active. (depends on: 1, 3.2)
- [x] **4.** Integrate the confirmation box into the existing overlay rendering path.
  - [x] **4.1** Add overlay layout/render plumbing for the confirmation prompt using the shared floating-window helpers. (depends on: 1)
  - [x] **4.2** Ensure the prompt remains non-focusable as a normal pane but still captures overlay input while open. (depends on: 4.1)

## Testing
- [x] **5.** Add widget-level tests for confirmation prompt behavior.
  - [x] **5.1** Verify `Y` and `Enter` return the supplied positive intent. (depends on: 1)
  - [x] **5.2** Verify `N` and `Esc` cancel without emitting an intent. (depends on: 1)
  - [x] **5.3** Verify unrelated keys do not close the prompt or emit stale intents. (depends on: 1)
- [x] **6.** Add integration tests for the quit-attempt flow.
  - [x] **6.1** Verify `<C-q>` quits immediately when no modified buffers exist. (depends on: 2, 3)
  - [x] **6.2** Verify `<C-q>` opens the confirmation prompt when modified buffers exist. (depends on: 2, 3)
  - [x] **6.3** Verify confirming the prompt proceeds with quit and declining leaves the editor open. (depends on: 1, 3)
- [x] **7.** Run `cargo check` and the relevant test targets to validate the new intent path, overlay behavior, and confirmation prompt rendering. (depends on: 1, 2, 3, 4, 5, 6)

## Documentation
- [x] **8.** Update architecture or motion/input documentation where needed to describe the quit-attempt flow and reusable confirmation prompt semantics. (depends on: 2, 3)

## Completion Summary
| Area | Total | Done | Remaining |
| --- | ---: | ---: | ---: |
| Backend | 10 | 10 | 0 |
| Testing | 7 | 7 | 0 |
| Documentation | 1 | 1 | 0 |
| Total | 18 | 18 | 0 |
