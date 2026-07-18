# Codex Assistant Native Smart Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to execute this plan one task at a time with a fresh implementer, an independent specification reviewer, and an independent code-quality reviewer before advancing.

**Goal:** Add an opt-in, per-root-conversation Smart Routing mode that configures verified Codex native custom agents, injects a visible control into the existing Codex composer, and routes bounded work to the least expensive eligible native model without weakening quality gates or opening another conversation window.

**Architecture:** Keep the existing Observer read-only and isolated. Add four new backend seams: a transactional Codex configurator, a metadata-only routing state/MCP service, a native capability preflight coordinator, and a hardened loopback CDP control layer. The injected composer control contains no routing engine and reads no task text; it only binds an opaque route key to an unambiguous local root and appends a visible, deterministic routing marker before submission. The installed routing skill performs native delegation, while the Observer proves lineage, effective model, lifecycle, and quality metadata.

**Tech Stack:** Tauri 2, Rust 2021/MSRV 1.82, React 19, TypeScript 7, Vitest/jsdom, Tokio, `toml_edit 0.22.27`, `reqwest 0.12.28`, `tokio-tungstenite 0.28.0`, `futures-util 0.3`, `windows-sys 0.59`, the existing SQLite/rollout Observer, and the Codex native custom-agent/MCP surfaces.

## Global Constraints

- The design spec at `docs/superpowers/specs/2026-07-18-codex-assistant-design.md` and ADR 0002 are normative. This plan implements only the native-routing phase; Savings and Themes remain later phases.
- Every delegated worker must be a real native Codex child under the visible root and must appear in Codex's native subagent panel. Detached `codex exec`, hidden tasks, second execution windows, fake child cards, and official binary/ASAR modification are forbidden.
- Quality is optimized before quota and time. Spark and Luna remain unavailable until requested/effective-model equality is proved for the exact direct or nested route on the current Codex/profile version.
- The router permits at most three active routed children per root, one depth-two child at a time, and two automatic escalations per subtask. Preserve an existing `agents.max_threads` exactly; set `agents.max_depth` to 2 only when absent or lower, preserving higher values.
- The only initially eligible visible route is `/local/:conversationId`, where the ID is a UUID and the Observer proves a root (`parent_thread_id = None`). `/remote`, `/work`, `/hotkey-window`, child-thread, ambiguous, and unknown routes fail closed.
- Persist no prompts, responses, reasoning, tool input/output, patches, commands, credentials, account identity, or full project paths. State and MCP schemas accept only opaque IDs, enumerated bands/reasons, effective model metadata, timestamps, counters, and booleans.
- The composer adapter may write only its deterministic visible marker/directive. It must not read, copy, log, or persist existing editor text. If DOM compatibility or insertion verification fails, the control becomes Degraded and submission proceeds without claiming routing is active.
- Configuration edits are ownership-scoped, idempotent, byte-backed-up, validated, same-directory atomically replaced, and rolled back on any failure. Never touch auth, provider, approval, sandbox, plugin, unrelated MCP, or unrelated agent settings.
- CDP must bind to a random loopback port, attach only to a verified Microsoft Store `OpenAI.Codex` executable owned by the current user, anchor the browser identity, reject non-loopback target URLs, and expose the same-user local-process risk in UI.
- Use dependency versions compatible with Rust 1.82. Do not add current `rmcp`, `toml_edit 0.25`, `reqwest 0.13`, or `tokio-tungstenite 0.30`.
- All state transitions have stable machine-readable reason codes and a user-readable explanation. Unsupported means unavailable, never silently substituted.
- Keep the worktree clean between tasks. Each task runs its focused tests, `npm run check`, and an independent review package before commit acceptance.

---

## Task 1: Define Routing Contracts, Persistent State, and Pure Quality-First Policy

**Files:**

- Create: `src-tauri/src/routing/mod.rs`
- Create: `src-tauri/src/routing/model.rs`
- Create: `src-tauri/src/routing/policy.rs`
- Create: `src-tauri/src/routing/state.rs`
- Create: `src-tauri/tests/routing_policy.rs`
- Create: `src-tauri/tests/routing_state_privacy.rs`
- Create: `shared/routing-types.ts`
- Create: `src/lib/routingApi.ts`
- Create: `src/lib/routingApi.test.ts`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**

```rust
pub enum ModelTier { Spark, Luna, Terra, Sol }
pub enum RouteKind { Direct, Nested }
pub enum ComplexityBand { Mechanical, Bounded, CrossLayer, Architectural }
pub enum RiskBand { Low, Meaningful, High, Restricted }
pub enum EligibilityStatus { Unknown, Verifying, Eligible, Unavailable, Stale }
pub enum RoutePhase { Off, Enabled, Classifying, Implementing, Reviewing, Completed, Degraded }

pub struct RoutePolicyInput {
    pub complexity: ComplexityBand,
    pub risk: RiskBand,
    pub required_capabilities: Vec<Capability>,
    pub eligible_tiers: Vec<ModelTier>,
    pub estimated_spawn_overhead_ms: u64,
    pub user_override: Option<UserOverride>,
}

pub struct RouteDecision {
    pub action: RouteAction,
    pub selected_tier: Option<ModelTier>,
    pub reviewer_floor: ModelTier,
    pub reason_codes: Vec<RouteReasonCode>,
}
```

The persisted envelope is versioned and contains `routes`, `eligibility`, `activity`, and `profile_version`. `RootRouteState` uses an opaque random route key and a conversation UUID; it never stores display titles or content. `RoutingSnapshot` is the only frontend payload.

### Steps

- [ ] Add failing Rust matrix tests proving: mechanical/low-risk work chooses Spark only when eligible; bounded work chooses Luna; cross-layer work chooses Terra; architecture, security, destructive, deployment, credential, or ambiguous work stays Sol/root; spawn overhead can keep trivial work in the parent; explicit `do not delegate` wins; explicit lower-tier overrides cannot bypass risk or capability floors.
- [ ] Add failing budget tests proving: no fourth active routed child, no second simultaneous nested child, no third automatic escalation, and no recursive reviewer fan-out.
- [ ] Add failing persistence/privacy tests that serialize representative route state, scan all keys/values, and reject content-like fields (`prompt`, `response`, `reasoning`, `command`, `patch`, `path`, `token`, `cookie`, `secret`). Verify corrupt state is quarantined and replaced with an empty versioned state without deleting the corrupt evidence.
- [ ] Add failing TypeScript parser tests proving unknown enums, missing schema version, extra content-bearing fields, and malformed UUIDs fail closed.
- [ ] Implement the Rust domain types, deterministic policy, budgets, mutex-protected `RoutingRuntime`, atomic `routing-state.json` storage under the preserved `codex-agent-monitor` settings directory, and sanitized frontend snapshot.
- [ ] Implement matching TypeScript discriminated unions and a strict hand-written parser; do not use unchecked casts at the Tauri boundary.
- [ ] Export `pub mod routing` from `lib.rs`, but do not add mutating Tauri commands yet.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test routing_policy --test routing_state_privacy`; expect all tests to pass.
- [ ] Run `npx vitest run src/lib/routingApi.test.ts`; expect all tests to pass.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit as `feat: add quality-first routing contracts`.

---

## Task 2: Install Codex-Owned Profiles, Skill, and Configuration Transactionally

**Files:**

- Create: `src-tauri/src/codex_config/mod.rs`
- Create: `src-tauri/src/codex_config/assets.rs`
- Create: `src-tauri/src/codex_config/transaction.rs`
- Create: `src-tauri/resources/routing/agents/spark.toml`
- Create: `src-tauri/resources/routing/agents/luna.toml`
- Create: `src-tauri/resources/routing/agents/terra.toml`
- Create: `src-tauri/resources/routing/agents/sol.toml`
- Create: `src-tauri/resources/routing/skill/SKILL.md`
- Create: `src-tauri/resources/routing/skill/references/policy.md`
- Create: `src-tauri/tests/codex_config_transaction.rs`
- Create: `src-tauri/tests/routing_assets.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/lib.rs`

**Owned config shape:**

```toml
[agents]
max_depth = 2

[agents.codex_assistant_spark]
description = "Mechanical, fully specified, low-risk native work"
config_file = "C:/Users/.../.codex/agents/codex-assistant/spark.toml"

[mcp_servers.codex_assistant_routing]
command = "C:/Program Files/Codex Assistant/codex-assistant.exe"
args = ["routing-mcp"]
enabled = true
required = false
enabled_tools = ["routing_policy_get", "routing_route_started", "routing_quality_record"]
```

Agent profile model slugs are exactly `gpt-5.3-codex-spark`, `gpt-5.6-luna`, `gpt-5.6-terra`, and `gpt-5.6-sol`. Every overridden spawn instruction uses `fork_turns="none"` or a bounded recent-history fork, never full-history inheritance with a model/profile override.

### Steps

- [ ] Pin `toml_edit = "=0.22.27"`; add all profile and skill resources to the Tauri bundle. Keep the package MSRV at 1.82 and verify `cargo tree` does not resolve `toml_edit 0.25`.
- [ ] Add failing fixture tests for empty, comment-heavy, CRLF, BOM-free, unrelated-agent, unrelated-MCP, existing-lower-depth, existing-higher-depth, existing/absent-max-threads, malformed, read-only, and injected-write-failure configs.
- [ ] Prove byte-identical rollback: after every injected failure point (backup creation, asset staging, config parse, temp sync, replace, post-write parse), `config.toml` and previously installed owned assets equal their pre-operation bytes.
- [ ] Implement discovery of the effective Codex home from Monitor settings/defaults, the canonical global skill root at `%USERPROFILE%\.agents\skills`, and the current executable path. Reject non-absolute paths, symlinks/reparse-point escapes, and asset destinations outside the exact owned directories.
- [ ] Implement versioned assets under `%USERPROFILE%\.codex\agents\codex-assistant\` and `%USERPROFILE%\.agents\skills\codex-assistant-smart-routing\`. Use generated absolute `config_file` paths and preserve unrelated files.
- [ ] Write the routing skill with exact classification, fan-out, self-verification, independent-review, repair, escalation, content-privacy, and native-only constraints from the design spec. It must query eligibility before spawning and must not claim unavailable profiles.
- [ ] Implement a transaction journal and timestamped backup in `%USERPROFILE%\.codex\codex-assistant-backups\`. Stage same-directory temp files, flush content, validate complete TOML/assets, atomically replace on Windows, and restore the complete preimage on failure.
- [ ] Merge only owned `[agents.codex_assistant_*]` and `[mcp_servers.codex_assistant_routing]` tables. Preserve `agents.max_threads` byte/value semantics; set `max_depth = max(existing, 2)` after enablement. Preserve all comments and unrelated ordering as far as `toml_edit` permits.
- [ ] Implement `inspect`, `install`, and `restore` services. `restore` removes only entries/assets whose recorded hashes still match the installed owned version; changed user files are retained and reported as conflicts.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test codex_config_transaction --test routing_assets`; expect all tests to pass.
- [ ] Run `cargo tree --manifest-path src-tauri/Cargo.toml -i toml_edit`; expect only `toml_edit v0.22.27` in the app's direct transaction path.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit as `feat: install native routing profiles safely`.

---

## Task 3: Add a Minimal Metadata-Only MCP Stdio Sidecar

**Files:**

- Create: `src-tauri/src/routing_mcp/mod.rs`
- Create: `src-tauri/src/routing_mcp/protocol.rs`
- Create: `src-tauri/src/routing_mcp/tools.rs`
- Create: `src-tauri/tests/routing_mcp_protocol.rs`
- Create: `src-tauri/tests/routing_mcp_privacy.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`

**Protocol surface:**

- `routing_policy_get(route_key)` returns eligibility, budgets, policy/profile versions, and reason-code vocabulary.
- `routing_route_started(route_key, child_thread_id, parent_thread_id, selected_profile, route_kind, complexity_band, risk_band, reason_codes)` records metadata only.
- `routing_quality_record(route_key, child_thread_id, outcome, reviewer_tier, retry_count, escalation_count)` records metadata only.

### Steps

- [ ] Add failing line-delimited JSON-RPC tests for `initialize`, `notifications/initialized`, `tools/list`, `tools/call`, unknown method, malformed ID, parse error, and graceful EOF. Negotiate only a supported MCP protocol version and return structured JSON-RPC errors.
- [ ] Add failing schema tests that reject unknown fields and explicit content canaries including `prompt`, `task`, `response`, `reasoning`, `tool_arguments`, `tool_output`, `patch`, `command`, `cwd`, `file_path`, `auth`, `cookie`, and `secret` at every nesting depth.
- [ ] Implement the stdio server directly with Tokio/Serde rather than `rmcp`. Reserve stdout exclusively for one JSON-RPC message per line; send diagnostics to stderr with sanitized codes and counters only.
- [ ] Make `main.rs` detect the exact `routing-mcp` subcommand before installing the GUI panic hook or starting Tauri. All other arguments continue through the normal single-instance GUI path.
- [ ] Reuse `RoutingRuntime`'s atomic state store with a cross-process lock file and bounded lock timeout. Reject unknown route keys, terminal child reactivation, budget overflow, non-enumerated models/reasons, and mismatched parent lineage claims pending Observer reconciliation.
- [ ] Add a subprocess test that starts the compiled binary in sidecar mode, completes initialize/tools/list/tool-call, asserts stdout contains no diagnostic prose, and verifies no task-content string reaches the state file or stderr.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test routing_mcp_protocol --test routing_mcp_privacy`; expect all tests to pass.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit as `feat: add metadata-only routing sidecar`.

---

## Task 4: Prove Native Direct and Nested Model Eligibility

**Files:**

- Create: `src-tauri/src/preflight/mod.rs`
- Create: `src-tauri/src/preflight/coordinator.rs`
- Create: `src-tauri/src/preflight/reconcile.rs`
- Create: `src-tauri/tests/preflight_reconcile.rs`
- Create: `src-tauri/tests/preflight_scope.rs`
- Modify: `src-tauri/src/monitor/model.rs`
- Modify: `src-tauri/src/monitor/reconcile.rs`
- Modify: `src-tauri/src/lib.rs`

**Eligibility key:**

```rust
pub struct EligibilityKey {
    pub codex_package_version: String,
    pub profile_version: String,
    pub requested_model: String,
    pub route_kind: RouteKind,
    pub depth: u8,
}
```

### Steps

- [ ] Add failing reconciliation fixtures for: true root→child equality, requested/effective drift, unrelated root, detached process, missing parent, duplicate candidate, terminal success, native model rejection, timeout, stale Codex version, and Terra→Luna/Spark depth-two lineage.
- [ ] Extend sanitized Observer metadata only where required to expose stable parent/root IDs, requested model, effective model, lifecycle, depth, and Codex package version. Add regression tests proving no content/tool fields enter `MonitorSnapshot`.
- [ ] Implement a coordinator state machine: `NotStarted → AwaitingVisibleCommand → AwaitingNativeChild → VerifyingLineage → Eligible|Unavailable|TimedOut`. Direct and nested keys are independent.
- [ ] Define deterministic visible preflight directives for each profile. Direct preflight asks the current root to spawn the exact owned profile with `fork_turns="none"`; nested preflight asks a verified Terra child to spawn one exact lower-tier profile. The directive contains no user task content.
- [ ] Mark a profile eligible only when one native child is observed under the expected root/parent, requested and effective model strings match exactly, the child reaches idle/terminal without availability failure, and no second root/detached process appears.
- [ ] Invalidate only affected keys on Codex/profile version changes or later native model availability/auth failures. Persist reason codes such as `effective_model_mismatch`, `native_profile_rejected`, `lineage_ambiguous`, and `host_version_changed`.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test preflight_reconcile --test preflight_scope`; expect all tests to pass.
- [ ] Run the existing monitor tests and `npm run check`; expect exit 0.
- [ ] Commit as `feat: verify native routing eligibility`.

---

## Task 5: Build the Verified Windows Store CDP Control Engine

**Files:**

- Create: `src-tauri/src/control_layer/mod.rs`
- Create: `src-tauri/src/control_layer/windows_package.rs`
- Create: `src-tauri/src/control_layer/cdp.rs`
- Create: `src-tauri/src/control_layer/injector.rs`
- Create: `src-tauri/tests/windows_package_identity.rs`
- Create: `src-tauri/tests/cdp_security.rs`
- Create: `src-tauri/tests/cdp_protocol.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`

### Steps

- [ ] Pin `reqwest = "=0.12.28"`, `tokio-tungstenite = "=0.28.0"`, `futures-util = "0.3"`, and `windows-sys = "0.59"` with the minimum Win32 feature set. Verify all selected versions support Rust 1.82.
- [ ] Add failing identity tests around abstracted process/listener probes: exact Store package family, canonical executable path beneath the reported package root, `ChatGPT.exe` basename, current-user process ownership, Authenticode success category, listener PID equality, and rejection of similarly named/untrusted executables.
- [ ] Add failing CDP fixtures that reject non-loopback `/json/version` or target websocket URLs, wrong ports, malformed Browser IDs, duplicate browser identities, identity changes after attach, unknown target types, oversized frames, and stale owned-session records.
- [ ] Implement package discovery through a narrowly scoped read-only Windows package query, then independently canonicalize and verify the executable. Use Windows process APIs to verify PID, owner SID, image path, and listener ownership before attaching.
- [ ] Select an ephemeral `127.0.0.1` port and launch the same verified Store executable with `--remote-debugging-address=127.0.0.1 --remote-debugging-port=<port>`. Never use `0.0.0.0`, a fixed public port, or shell-built command strings.
- [ ] Refuse restart while the Observer reports any active native child or unsent setup phase. Close only the exact verified current Store PID, wait for exit, launch one replacement, and prove there is no second Codex UI process.
- [ ] Implement a single-reader/single-writer CDP client with monotonically increasing request IDs, bounded timeouts/frame sizes, Browser websocket identity anchoring, target attach/detach reconciliation, and sanitized error categories.
- [ ] Implement `Runtime.enable`, `Page.enable`, `Runtime.addBinding`, `Page.addScriptToEvaluateOnNewDocument`, and `Runtime.evaluate` primitives. Reinject after navigation/target recreation; tear down immediately if browser identity changes.
- [ ] Persist only an owned-session record containing port, verified PID, Browser ID hash, Codex version, start time, and engine version. Do not store websocket URLs after connection or any page payload.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test windows_package_identity --test cdp_security --test cdp_protocol`; expect all tests to pass.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit as `feat: add verified Codex CDP control layer`.

---

## Task 6: Inject a Fail-Closed Per-Conversation Smart Routing Control

**Files:**

- Create: `src-tauri/resources/control/routing-control.js`
- Create: `src-tauri/resources/control/routing-control.css`
- Create: `src-tauri/resources/control/fixtures/local-root.html`
- Create: `src-tauri/resources/control/fixtures/local-child.html`
- Create: `src-tauri/resources/control/fixtures/incompatible.html`
- Create: `src/control/routingControlHarness.ts`
- Create: `src/control/routingControlHarness.test.ts`
- Create: `src-tauri/tests/control_asset_contract.rs`
- Modify: `src-tauri/src/control_layer/injector.rs`
- Modify: `src-tauri/tauri.conf.json`

**Visible marker:**

```text
[Codex Assistant Routing v1; route=<opaque-uuid>; policy=1]
Use $codex-assistant-smart-routing for eligible bounded delegation in this turn. Keep all children native to this root, enforce eligibility/budgets/quality review, and report actual effective models.
```

### Steps

- [ ] Add jsdom fixture tests for `/local/<uuid>`, local child, `/remote`, `/work/conversation`, `/hotkey-window/thread`, malformed UUID, missing shell markers, duplicate composers, active modal/menu, IME composition, Shift+Enter, configured multiline shortcuts, send button, stop button, disabled button, repeated turns, enable/disable, navigation, and reinjection idempotency.
- [ ] Make the compatibility probe require `main.main-surface`, `aside.app-shell-left-panel`, `[data-codex-composer-root]`, `[data-codex-composer="true"]`, and `.ProseMirror`. Bind only when Rust confirms the route UUID is an observed local root with `parent_thread_id = None`.
- [ ] Inject exactly one namespaced chip using `data-codex-assistant-control`, plus scoped CSS under a namespaced root. Do not modify existing nodes except adding/removing the deterministic marker through the editor's supported input transaction.
- [ ] Register a CDP `Runtime.addBinding` channel for `toggle`, `compatibility`, `submit_intent`, and `insertion_result`. Validate every message against an exact schema and target session; injected JavaScript may not open sockets or call Tauri directly.
- [ ] On enable, show `Enabled` and the current route's opaque key. On route activity, accept Rust-pushed sanitized states only: Classifying, model/implementing, escalation, Reviewing, Completed, Degraded/unavailable.
- [ ] Append the marker synchronously before a genuine composer submission from either the active keyboard shortcut or the current submit control. Use the current stable composer root/input selectors plus structural button validation; never match dialogs or request forms outside the root.
- [ ] Verify insertion by inspecting only the exact inserted range/transaction result, not by reading or returning the editor's pre-existing text. If insertion cannot be proved before the host submit handler, cancel the assistant mutation, emit `insertion_failed`, show Degraded, and do not claim the turn is routed.
- [ ] Prevent duplicate markers in the same submission transaction; repeat one marker on every later enabled turn. Disabling prevents future insertion but does not stop active native children.
- [ ] Add static asset-contract tests forbidding `fetch`, `XMLHttpRequest`, `WebSocket`, `eval`, remote URLs, localStorage/sessionStorage, clipboard APIs, and generic editor serialization. Allow only the engine-owned CDP binding and namespaced globals.
- [ ] Run `npx vitest run src/control/routingControlHarness.test.ts`; expect all tests to pass.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test control_asset_contract`; expect all tests to pass.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit as `feat: inject per-conversation smart routing control`.

---

## Task 7: Add the Smart Routing Setup and Status Experience

**Files:**

- Create: `src/components/AppNavigation.tsx`
- Create: `src/components/AppNavigation.test.tsx`
- Create: `src/components/SmartRoutingPage.tsx`
- Create: `src/components/SmartRoutingPage.test.tsx`
- Create: `src/hooks/useRouting.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/styles/global.css`
- Modify: `src/lib/routingApi.ts`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/permissions/default.toml`
- Modify: `src-tauri/tests/acl_consistency.rs`

**Tauri command surface:**

- `get_routing_snapshot()`
- `install_routing()`
- `restore_routing()`
- `request_codex_restart()`
- `begin_routing_preflight(root_conversation_id)`
- `set_root_routing_enabled(root_conversation_id, enabled)`

### Steps

- [ ] Add failing ACL tests requiring exact agreement among invoke handler, permissions file, capabilities, Rust command functions, and frontend invocations. Keep commands narrow; no generic file/process/script/CDP command may cross the frontend boundary.
- [ ] Add failing component tests for uninstalled, install pending, restart required, restart blocked by active child, preflight running, mixed profile availability, direct-only Luna, nested Luna, unsupported Spark/Luna, CDP degraded, enabled route, disabling with active child, config conflict, and Restore confirmation states.
- [ ] Implement a two-area shell with `Live Agents` and `Smart Routing`. Keep current Live Agents behavior and tests unchanged behind the first navigation item.
- [ ] Implement setup status, exact configuration diff summary, backup location, same-user CDP risk disclosure, one-time restart state, and Restore. Installation must not automatically restart until config validation succeeds and the Observer reports idle.
- [ ] Present Spark/Luna/Terra/Sol rows with requested model, actual effective model, direct eligibility, nested eligibility, last verified Codex/profile version, and a specific reason when unavailable. Root-picker presence alone must not render eligible.
- [ ] Present the quality-first routing matrix, fan-out/depth/escalation limits, privacy contract, and current root routes. Do not add savings percentages or quota claims in this phase.
- [ ] Implement `useRouting` with event-driven snapshots and bounded polling fallback. Parse all backend payloads strictly and retain the last known good snapshot on malformed events while showing Degraded.
- [ ] Register the exact Tauri commands and update permissions. Mutating commands return sanitized operation receipts with operation ID, status, reason codes, and whether restart is required; they never expose raw config or process command lines.
- [ ] Run `npx vitest run src/components/AppNavigation.test.tsx src/components/SmartRoutingPage.test.tsx src/App.test.tsx`; expect all tests to pass.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test acl_consistency`; expect all tests to pass.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit as `feat: add smart routing setup and status`.

---

## Task 8: Exercise the One-Time Restart and Real Native Host Preflight

**Files:**

- Create: `scripts/routing-integration-smoke.ps1`
- Create: `docs/testing/native-routing-smoke.md`
- Create: `.superpowers/sdd/routing-host-smoke-report.md`
- Modify: `src-tauri/src/control_layer/windows_package.rs`
- Modify: `src-tauri/src/control_layer/injector.rs`
- Modify: `src-tauri/src/preflight/coordinator.rs`
- Modify: `src-tauri/resources/control/routing-control.js`

### Steps

- [ ] Build a temporary-home smoke harness that copies representative `config.toml` fixtures, runs inspect/install/reinstall/restore, injects each transaction failure point, and proves byte-equal rollback and no changes outside the temporary Codex/skill roots.
- [ ] Run the harness without touching the real Codex home. Expected result: install, idempotent reinstall, conflict-safe restore, and every failure rollback pass.
- [ ] Record the current Store package version, Codex CLI/runner versions, installed profile version, and Observer idle state in a sanitized report. Do not record account identity, prompt content, or project paths.
- [ ] Wait until the Observer proves no active child. Back up the exact real config and owned destinations, install routing assets/config, validate them, and request the single accepted Codex restart through the verified control layer.
- [ ] Prove exactly one official Codex window/process returns on a random loopback CDP port, Browser identity remains anchored, official files are unchanged, and the injected chip appears only on the current `/local/<root-uuid>` composer.
- [ ] Trigger the visible direct preflight for Spark, Luna, Terra, and Sol. For each, record observed lineage, requested/effective model, lifecycle, and final eligibility. Do not convert a mismatch/fallback into success.
- [ ] If direct Terra and a lower tier are eligible, run one depth-two Terra→lower-tier preflight with one nested child. If the host rejects model/profile/depth, record the exact sanitized reason and keep nested eligibility unavailable.
- [ ] Send at least two later turns with Smart Routing enabled and prove the marker repeats, all delegated children remain in the same native panel/root, budget counters hold, and no second conversation/window opens. Then disable and prove a subsequent turn receives no marker/new routed child while an already-running child is allowed to finish.
- [ ] Run compatibility recovery by removing the injected node through CDP and navigating away/back; expect idempotent reinjection. Simulate a DOM-probe failure in the fixture only; expect Degraded and no marker.
- [ ] Restore the pre-smoke user state if the integration fails before acceptance. On success, keep the user-enabled configuration and retain the timestamped rollback backup.
- [ ] Complete `.superpowers/sdd/routing-host-smoke-report.md` with commands, versions, availability outcomes, evidence hashes, and sanitized limitations.
- [ ] Run `npm run check`; expect exit 0.
- [ ] Commit code/docs changes as `test: verify native smart routing host integration`. Do not commit machine-specific config, PIDs, route keys, package paths, or user identifiers.

---

## Task 9: Release-Harden Routing and Close the Phase

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `THIRD_PARTY_NOTICES.md` only if a new distributed dependency requires notice
- Modify: `docs/superpowers/specs/2026-07-18-codex-assistant-design.md`
- Modify: `src-tauri/installer-hooks.nsh`
- Modify: `src-tauri/tests/installer_hooks.rs`
- Modify: `.superpowers/sdd/routing-progress.md`
- Create: `.superpowers/sdd/routing-final-review.md`

### Steps

- [ ] Fix the two deferred identity-release minors while the installer is being rebuilt: reject a directory masquerading as the legacy executable/uninstaller by checking file attributes, and replace basename-wide legacy process termination with verified install-path/PID termination. Add both NSIS regression fixtures.
- [ ] Document the exact verified/unavailable profile outcomes from the real preflight, the native-only guarantee, quality gates, max-depth/fan-out limits, one-time restart, Restore path, privacy boundary, and same-user loopback CDP risk.
- [ ] Update the design status for the native-routing phase only. Do not mark Savings, Themes, or public deployment complete.
- [ ] Run focused frontend/Rust tests, `npm run check`, `npm run build`, and `npm run tauri build`; expect exit 0 and a `Codex Assistant_0.5.0_x64-setup.exe` artifact.
- [ ] Run the existing fresh-install, 0.4→0.5 upgrade, repeat-install, injected-failure rollback, and uninstall smoke flows. Add routing-config preservation/Restore checks without deleting user-modified owned files.
- [ ] Generate a bounded review diff from the routing-plan merge base and dispatch an independent specification review plus code-quality/security review. Fix every Critical/Important finding and re-run both reviews.
- [ ] Dispatch a final Sol whole-branch review covering native lineage truthfulness, privacy, configuration rollback, CDP process identity, DOM fail-closed behavior, ACLs, release installer, and documentation. Acceptance requires Critical 0 and Important 0.
- [ ] Update `.superpowers/sdd/routing-final-review.md` with final commit range, exact tests/builds, real-host outcomes, supported/unavailable models, residual non-blocking risks, and reviewer verdict.
- [ ] Update `D:\Work_plan\README.md` once with the completed native-routing phase, using Asia/Shanghai date and verified facts only.
- [ ] Commit as `release: complete native smart routing phase`.

## Phase Exit Criteria

- One validated setup transaction installs owned native profiles, routing skill, and MCP entry, retains a byte-level rollback path, and restarts Codex only once while idle.
- The visible Smart Routing chip binds only to one proven local root and persists through multiple later turns until disabled.
- Every routed execution is a genuine native child in that root's Codex subagent panel. No external executor, hidden task, second window, or simulated child is used.
- Direct and nested Spark/Luna eligibility reflects actual requested/effective model equality on the current host; unsupported tiers are visibly unavailable.
- Routing honors risk/capability floors, spawn-overhead decisions, fan-out/depth/escalation budgets, self-verification, independent review, repair, and escalation.
- Observer, routing state, MCP, frontend payloads, logs, and smoke artifacts remain content-free by contract and test.
- CDP is loopback-only, exact-process verified, identity-anchored, update-sensitive, and fail-closed; official Codex files remain unchanged.
- All checks, release build, installer migration/rollback smokes, independent reviews, and the work log are complete. Savings, Themes, and public deployment remain explicitly pending phases.
