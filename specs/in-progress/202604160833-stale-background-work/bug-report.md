# 202604160833-stale-background-work: Cancel stale background work
## Summary
Rapid editing can create a backlog of background work, especially syntax highlighting jobs. Older work keeps running even after it has been made obsolete by newer edits, which wastes CPU time and delays fresh results.

## Severity: Medium

## Environment
- urvim editor
- Background job framework
- Syntax highlighting worker path

## Reproduction Steps
1. Open a large buffer with syntax highlighting enabled.
2. Make a series of fast edits in the same region, such as holding a key to insert or delete many characters.
3. Keep typing before the background worker finishes the earlier highlight passes.
4. Observe that older highlight work remains queued or continues executing even though newer edits have already superseded it.

## Expected Behavior
- Newer background work should supersede older stale work for the same target.
- The worker should cancel or skip obsolete jobs instead of letting them drain the queue.
- The editor should keep showing the last completed highlight state until fresher work finishes.

## Actual Behavior
- Background syntax work accumulates behind the current edit stream.
- The worker continues spending time on older jobs that no longer match the buffer state.
- Fresh highlight updates arrive later than necessary because stale work is still consuming worker time.

## Impact
- Increased CPU usage during rapid editing.
- Highlighting can lag behind the user’s current buffer state.
- The backlog can make the editor feel less responsive even if the main input loop remains live.

## Root Cause
The background job framework does not treat queued syntax work as replaceable. Once jobs are submitted, older work is still allowed to run even when newer edits have already invalidated it. The system lacks a general stale-work cancellation or supersession mechanism, so the worker can only catch up by processing obsolete jobs one by one.

## Solution Approach
Add a general cancellation or supersession mechanism for background jobs so newer work can invalidate older queued work for the same scope. For syntax highlighting, only the latest job for a buffer or equivalent unit should remain eligible to run. The main thread should continue displaying the last completed highlight result while the new job is pending.

Rejected alternatives:
- Waiting for the queue to drain naturally, which preserves the backlog and the lag.
- Clearing the rendered syntax state immediately when work becomes stale, which is more disruptive than keeping the last good result visible.

## Code Changes
- Update the background job framework to carry a cancellation or generation token for replaceable work.
- Teach the syntax highlighting worker path to invalidate older queued jobs when a newer edit arrives.
- Ensure worker-side checks skip obsolete jobs before expensive processing and before result application.
- Add or update tests around rapid edit bursts and stale-job cancellation behavior.

## Edge Cases
- Multiple rapid edits that arrive before the worker starts any highlight pass.
- Jobs that become stale while they are already executing.
- Buffers that are switched or closed while background work is pending.
- Other future background tasks that should use the same stale-work cancellation behavior.
