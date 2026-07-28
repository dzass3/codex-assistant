# ADR 0008: Process-session-bounded agent liveness

- Status: Accepted
- Date: 2026-07-27

Unmatched historical task-start records can survive for days and were previously shown as green running work with an old relative time. Codex Assistant now treats the current verified official Codex process lifetime as the boundary for live status and uses the latest whitelisted activity evidence, not task creation time, as the displayed age.

Events older than the current process session cannot establish current liveness without newer rollout or state evidence. Unclosed older records remain visible only in the complete history as `历史状态未闭合`; ambiguous records in the current session are `状态待确认` and continue to block a safe restart. Backend timestamps remain UTC epoch milliseconds and the frontend renders full times in the Windows locale and time zone.

This preserves historical observability without presenting stale work as live. It also deliberately makes restart safety more conservative than the active-work display when evidence is uncertain.
