# ADR 0006: Read-only observer with safe themes

- Status: Accepted
- Date: 2026-07-22

## Context

Theme-only Codex Assistant removed risky routing and injected task-control behavior, but it also removed useful visibility into native subagents and the actual model running each task. Restart safety still depended on hidden monitor state, so users could not compare the visible impact with the guard that protected their work.

The observer can be restored without restoring routing if its contract is limited to existing local metadata and its command surface remains read-only. Theme safety also needs explicit page classification because a selector match alone is not enough to distinguish a main task from account, authorization or other sensitive screens.

## Decision

Codex Assistant exposes exactly two top-level pages: `实时代理` and `一键换肤`. First launch opens themes; a namespaced local preference remembers later page selection. There is no Smart Routing page, agent action, routing profile, routing command or packaged routing runtime.

The observer reads the existing Codex state database and rollout metadata and publishes only a strict whitelist: opaque thread relationship, safe display label, role/project/originator, requested and effective model, source, effort, lifecycle, depth and bounded timestamps. Prompt, response, reasoning, tool content and full paths are rejected. Requested-only intent is never represented as an effective model. Active-only view is the default; idle and interrupted work remains available through an explicit `全部` filter.

The restart guard consumes a projection from the same reconciled snapshot: known starting/running count plus monitor confidence. Known work returns `active-work`; degraded sources or tracking errors return `monitor-uncertain`. Normal restart fails closed for either. A force ticket binds count, confidence, process identity, intent and subject, and expires after 60 seconds.

Before rich theme injection, a bounded structural classifier identifies `main-task`, `utility`, `sensitive` or `unknown`. Password, authorization, permission, recovery and security evidence wins before main-page evidence. Rich styles require visible, hit-testable main and sidebar anchors plus a visible composer or home-state anchor. Utility, sensitive and unknown pages remain official. A tiny owned observer disables the one owned stylesheet if an already-themed single-page application enters an incompatible state; it never reads or changes semantic content.

## Consequences

- Users regain truthful native-agent visibility without gaining an agent-control surface.
- Observer polling remains read-only and can never apply a theme or restart Codex.
- Degraded observation reduces restart convenience but increases caution.
- Switching themes inside a verified main-task session remains one click and does not require a restart.
- Sensitive and unknown pages cannot receive a rich background even if generic shell elements happen to exist.
- ADR 0004 remains authoritative for removal of routing and safe theme ownership, but its theme-only product-surface decision is superseded.
- ADR 0005 remains authoritative for the unchanged official entry and manual reapply after a complete official-app reopen.
