# Codex Agent Monitor — Domain Context

## Glossary

### Root thread

A user-visible Codex task that owns zero or more descendant agent threads.

### Agent thread

A Codex execution thread with its own identity, model settings, lifecycle events, and optional parent thread.

### Subagent

An agent thread created by another agent thread. A subagent remains the same agent even when it moves between running and idle turns.

### Requested model

The model explicitly supplied when the parent asks Codex to create a subagent. It records intent and can be absent when inheritance is requested.

### Effective model

The model recorded in the subagent's latest `turn_context`. This is the authoritative answer to “which model is this subagent actually using?” and can differ from the requested model.

### Reasoning effort

The effective reasoning-strength setting recorded for the subagent's latest turn.

### Observation

A metadata-only view of one agent thread at a point in time. It contains identity, lineage, model, reasoning effort, lifecycle state, timestamps, and provenance, but no conversation or tool content.

### Observation freshness

The age of the newest trusted metadata event used to construct an observation.

### Lifecycle state

One of `starting`, `running`, `idle`, `interrupted`, or `tracking-error`. `Idle` means the most recent turn completed and the agent may still receive a follow-up; it does not mean the agent was destroyed.

### Model drift

A condition where the requested model and effective model are both known and differ.

### Metadata-only boundary

The product rule that conversation bodies, reasoning text, tool arguments, tool outputs, full filesystem paths, authentication material, and secrets are never retained, logged, sent to the frontend, or displayed.

## Invariants

- Effective model wins over requested model whenever both exist.
- A requested model without an effective model is displayed as pending confirmation, not as actual usage.
- Lifecycle state is derived from task and activity events; the persisted spawn-edge `open` value is not treated as proof that an agent is running.
- All Codex data sources are opened read-only.
- Project identity is shown as a directory basename unless the user explicitly reveals the full path.
- Monitoring failure must never alter Codex state.
