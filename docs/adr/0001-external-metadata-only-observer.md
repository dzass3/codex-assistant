# ADR 0001: External metadata-only observer

- Status: Accepted
- Date: 2026-07-18

## Context

Codex Desktop exposes subagent activity but does not consistently show each subagent's effective model. The local Codex state database and rollout files contain enough metadata to reconstruct agent lineage and effective model without modifying the Codex application. Requested model and effective model can differ, so observing only the spawn request is insufficient.

The monitor must work with the installed Windows Codex application, survive Codex UI updates, and avoid exposing conversation content or credentials.

## Decision

Build a separate Windows desktop application. It opens `state_5.sqlite` in read-only mode for the initial agent graph and incrementally watches rollout JSONL files for authoritative model and lifecycle metadata.

The application uses a whitelist parser. It retains and emits only agent identity, parent relationship, requested/effective model, reasoning effort, lifecycle timestamps, project basename, source, and parser health. It does not persist a copy of Codex conversations and does not read `auth.json`.

The effective model is taken from the child thread's latest `turn_context.model`. Spawn arguments are retained only as requested-model intent.

## Consequences

- The monitor works without injecting code into Codex Desktop or proxying model traffic.
- A Codex schema change can temporarily reduce visibility, so data-source health and provenance must be visible.
- Some rollout lines necessarily pass through a short-lived line buffer before type filtering, but non-whitelisted payloads are not deserialized, retained, logged, sent to the frontend, or displayed.
- The product intentionally omits transcript viewing, prompt inspection, command output, and file-change analytics.
- The existing Codex Trace project can supply proven Tauri, file-watching, and packaging foundations, but content-viewer features are outside this product and must not remain reachable.

## Alternatives considered

### Embed into Codex Desktop

Rejected because the desktop UI has no supported extension surface for this feature and updates could break injection or patching.

### Port Codex HUD

Rejected because its primary interface depends on terminal and tmux workflows, while the target is native Windows Codex Desktop.

### Capture network or process memory

Rejected because it is invasive, fragile, and unnecessary when local authoritative metadata is already available.
