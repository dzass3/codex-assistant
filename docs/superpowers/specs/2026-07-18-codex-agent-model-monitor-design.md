# Codex Agent Model Monitor — Design

## Summary

Codex Agent Monitor is a native Windows companion that shows which models Codex subagents are actually using in real time. It is a separate, read-only window built from the MIT-licensed Codex Trace Tauri foundation and narrowed to metadata-only observability.

The first release monitors all local Codex surfaces that share the selected `CODEX_HOME`, including Desktop, CLI, and IDE-created threads. It labels the source when known and defaults to the current user's `~/.codex` directory.

## Goals

- Show root and descendant agent threads as a live hierarchy.
- Show requested model, authoritative effective model, reasoning effort, and model drift.
- Distinguish starting, running, idle, interrupted, and tracking-error states.
- Work with the existing Windows Codex installation without modifying it.
- Remain local-first and metadata-only.
- Produce an installable Windows application and a deployed Sites download page.

## Non-goals

- Rendering prompts, responses, reasoning, tool calls, command output, patches, or changed files.
- Reading or managing Codex authentication and quota information.
- Changing models, interrupting agents, sending follow-ups, or otherwise controlling Codex.
- Guaranteeing compatibility with undocumented future storage formats without an update.
- Replacing Codex Desktop's task interface.

## Approaches considered

### 1. Focused Tauri application based on Codex Trace — selected

Reuse the upstream Rust/Tauri/React build, watcher, single-instance, and Windows packaging foundations. Replace the conversation-oriented surface with a purpose-built metadata monitor. This gives the best Windows experience while avoiding a new platform stack.

### 2. Extend Codex HUD

Its active-agent logic is useful, but the tmux-oriented UI and WSL-only Windows posture do not fit a native Codex Desktop companion.

### 3. Greenfield Windows application

A new .NET or Tauri application would provide maximum isolation but duplicate mature file-watching, packaging, and cross-platform infrastructure. The selected approach keeps isolation at the parser and product-surface boundaries instead.

## Architecture

### Metadata collector

The Rust backend owns all filesystem and SQLite access. It discovers `CODEX_HOME` from the environment or the current user profile and supports a user-selected override.

At startup it opens `state_5.sqlite` with SQLite read-only flags and reads only these logical fields:

- thread identity, parent/child spawn edges, and agent path;
- nickname, role, origin, model, reasoning effort, and update timestamps;
- rollout path and project directory, reduced to a basename before frontend delivery.

The database produces a fast initial graph. The collector never writes to the database and does not use `logs_2.sqlite`.

### Whitelist rollout observer

The observer watches active session directories and keeps an in-memory byte cursor for each relevant rollout file. Before deserialization, each line is classified by a narrow textual envelope check. Only these records are eligible for structured parsing:

- `session_meta` for safe agent identity and lineage fields;
- `turn_context` for effective model and reasoning effort;
- `task_started` and `task_complete` for turn state;
- `sub_agent_activity` for start, interaction, and interruption signals;
- `spawn_agent` function calls for requested model, effort, and task name.

All other records are discarded after the transient line read. Raw lines and non-whitelisted payloads are never logged, persisted, or sent to the frontend.

### Reconciliation engine

The engine merges database rows and rollout observations into immutable `AgentObservation` snapshots keyed by thread ID.

Precedence rules:

1. Latest child `turn_context.model` is the effective model.
2. Database thread model is a fallback effective-model candidate with explicit provenance.
3. Parent `spawn_agent` model is requested intent only.
4. Effective reasoning effort follows the same rollout-over-database precedence.
5. Parent/child edges are unioned from SQLite and rollout events.

The engine detects model drift when known requested and effective models differ.

### Lifecycle inference

- `starting`: spawn edge exists but the child has not emitted a usable turn context or task start.
- `running`: the newest lifecycle boundary is a task start, or a newer interaction belongs to that active turn.
- `idle`: the newest lifecycle boundary is task completion. The agent remains available for follow-up.
- `interrupted`: the newest authoritative activity signal is interruption.
- `tracking-error`: required metadata cannot be read or reconciled safely.

The SQLite spawn-edge status is not used as a running-state signal because current Codex versions can leave completed edges marked `open`.

### Frontend

React receives complete snapshots through a Tauri command for initial load and a debounced Tauri event for updates. It has no direct filesystem permission.

The main window contains:

- a compact summary strip for running, starting, idle, drift, and error counts;
- source-health indicators for the state database and rollout watcher;
- an expandable agent tree grouped by root thread;
- model badges that visually prioritize effective model;
- a requested-model badge only when pending or different;
- reasoning effort, status, elapsed time, observation freshness, source, and project basename;
- filters for active/all, model, source, and project;
- a manual refresh action and settings for `CODEX_HOME`.

No transcript or tool-content view is included. The existing conversation viewer is removed from the reachable product surface and unused content-oriented dependencies are removed.

## Error handling

- Missing `state_5.sqlite`: continue in rollout-only degraded mode.
- Busy or migrating database: retain the last snapshot and retry with bounded backoff.
- Malformed JSONL line: skip it, count a sanitized parse error, and continue tailing.
- Truncated or rotated rollout: reset that file cursor safely and rescan whitelisted metadata.
- Unknown model or effort: display `Unknown`, never infer from the parent UI selection.
- Watcher overflow: perform a bounded reconciliation scan and report temporary degraded health.
- Invalid custom directory: keep the last valid directory and show a local validation message.

## Privacy and security

- All Codex sources are opened read-only.
- No network request is needed for monitoring.
- `auth.json`, `logs_2.sqlite`, prompts, responses, reasoning, and tool payloads are out of scope.
- The application keeps observations in memory and has no analytics or telemetry.
- Frontend payload types contain no raw path or raw JSON field.
- Diagnostics contain counters, schema versions, and sanitized error categories only.

## Testing

### Rust unit tests

- Whitelist rejects conversation and tool-output records.
- Requested and effective model precedence.
- Model-drift detection.
- Lifecycle transitions and follow-up turns.
- SQLite read-only projection and missing-column degradation.
- Incremental tailing, truncation, malformed lines, and duplicate events.

### Rust integration tests

Use temporary synthetic `CODEX_HOME` fixtures containing a minimal state database and rollout files. Assert exact snapshots and verify source files remain byte-identical.

### Frontend tests

- Tree nesting and expansion.
- Effective/requested model presentation.
- Drift and tracking-error accessibility labels.
- Active/all and model filters.
- Empty, degraded, and unknown-model states.

### End-to-end acceptance

Run against a sanitized fixture and the local Codex metadata source. Confirm that a newly spawned test subagent appears, transitions to running and idle, and displays the same effective model and effort as its own latest `turn_context`.

Run formatting, lint, type checking, Rust tests, frontend tests, release build, and installer smoke checks before delivery.

## Packaging and attribution

- Product name: `Codex Agent Monitor`.
- Windows artifacts: NSIS installer and portable executable when supported by the Tauri build.
- Preserve the upstream MIT license and add clear Codex Trace attribution.
- Keep the upstream remote for comparison; local product work lives on the `codex-agent-model-monitor` branch.

## Deployment

After the desktop release passes acceptance:

1. Create a one-page Sites project inside the repository's `site` directory.
2. Present the product purpose, privacy guarantees, supported fields, installation steps, and version.
3. Include the validated Windows installer as the download target when deployment size permits; otherwise link to the local release artifact handoff and deploy the documentation page.
4. Build and deploy privately through Sites by default.
5. Return the deployed URL as the primary web deliverable and the local installer as the primary desktop deliverable.

## Acceptance criteria

- A user can launch a standalone Windows window without changing Codex configuration.
- Every discoverable subagent row shows its effective model or an explicit unknown/pending state.
- Requested and effective models are separately labeled and drift is visible.
- A running subagent update appears without manual reload under normal watcher operation.
- Closing or restarting the monitor does not change Codex files.
- No frontend event, diagnostic record, or product view contains conversation or tool content.
- Windows installer launches successfully on the target machine.
- The Sites page builds successfully and is deployed with a working product description and download handoff.
