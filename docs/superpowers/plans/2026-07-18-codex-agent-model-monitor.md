# Codex Agent Model Monitor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver an installable Windows companion that shows every local Codex subagent's effective model, requested model, reasoning effort, hierarchy, and live lifecycle state without exposing conversation or tool content.

**Architecture:** A Rust backend opens Codex state in read-only mode, incrementally extracts a strict metadata whitelist from rollout files, and reconciles both sources into immutable snapshots. A React/Tauri frontend renders those snapshots as a filterable agent tree. A separate Sites page publishes the validated installer and privacy/usage documentation.

**Tech Stack:** Rust 2021, Tauri 2, rusqlite, serde/serde_json, Tokio, React 19, TypeScript 7, Vite 8, Vitest, Sites/vinext.

## Global Constraints

- Product name is `Codex Agent Monitor` and bundle identifier is `com.codexagentmonitor.desktop`.
- Support native Windows first; preserve the upstream Rust 1.77.2 floor unless a dependency proves incompatible.
- Open `state_5.sqlite` and rollout files read-only; never write into `CODEX_HOME`.
- Never open `auth.json` or `logs_2.sqlite`.
- Never retain, log, emit, or display prompts, responses, reasoning text, tool arguments, tool outputs, full filesystem paths, or raw JSON.
- `turn_context.model` is authoritative; database model is fallback; `spawn_agent` model is requested intent only.
- Show project directory basename only.
- Do not use `thread_spawn_edges.status` as proof that an agent is running.
- Every source/config change receives tests and passes format, lint, type check, Rust tests, frontend tests, release build, and installer smoke check.

---

## File Structure

### Rust backend

- `src-tauri/src/monitor/mod.rs` — public monitor module and exports.
- `src-tauri/src/monitor/model.rs` — serialized domain types and sanitized health types.
- `src-tauri/src/monitor/reconcile.rs` — deterministic precedence, drift, tree depth, and lifecycle logic.
- `src-tauri/src/monitor/sqlite_source.rs` — read-only state database projection.
- `src-tauri/src/monitor/rollout_source.rs` — whitelist line classification and incremental rollout facts.
- `src-tauri/src/monitor/runtime.rs` — source refresh loop, cached snapshot, settings, and Tauri events.
- `src-tauri/src/lib.rs` — minimal desktop boot and monitor commands.

### Frontend

- `shared/monitor-types.ts` — frontend contract matching Rust serialized names.
- `src/lib/monitorApi.ts` — Tauri command/event adapter.
- `src/hooks/useMonitor.ts` — snapshot lifecycle and reconnect behavior.
- `src/components/AgentTree.tsx` — root grouping and recursive tree.
- `src/components/AgentRow.tsx` — one metadata-only agent row.
- `src/components/FilterBar.tsx` — active/all, model, source, project, and search filters.
- `src/components/HealthStrip.tsx` — counts and source health.
- `src/components/SettingsDialog.tsx` — validated `CODEX_HOME` override.
- `src/App.tsx` — monitor composition only.
- `src/styles/global.css` — focused responsive monitor styling.

### Validation and delivery

- `src-tauri/tests/monitor_fixture.rs` — synthetic `CODEX_HOME` integration tests.
- `README.md` — install, privacy contract, source precedence, and attribution.
- `THIRD_PARTY_NOTICES.md` — Codex Trace MIT attribution.
- `site/` — Sites download and documentation page created only after the installer passes.

---

### Task 1: Rebrand and lock down the desktop surface

**Files:**
- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/src/main.rs`
- Create: `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Produces: a single-instance native app named `Codex Agent Monitor` with no frontend filesystem permission.

- [ ] **Step 1: Add a configuration test**

Create `src/config.test.ts` that reads exported constants from `src/config.ts` and asserts:

```ts
expect(PRODUCT_NAME).toBe("Codex Agent Monitor");
expect(MONITOR_EVENT).toBe("monitor://snapshot");
```

- [ ] **Step 2: Run the focused test and verify failure**

Run: `npx vitest run src/config.test.ts`

Expected: FAIL because `src/config.ts` does not exist.

- [ ] **Step 3: Add product constants and rebrand manifests**

Create `src/config.ts`:

```ts
export const PRODUCT_NAME = "Codex Agent Monitor";
export const MONITOR_EVENT = "monitor://snapshot";
export const DEFAULT_REFRESH_MS = 1000;
```

Set the npm package and Rust crate to `codex-agent-model-monitor`, set the Tauri product/title/identifier, remove HTTP/web-only binaries, and reduce Tauri capabilities to `core:default` plus `core:event:default`. Preserve the upstream MIT license and record the exact upstream repository and base commit in `THIRD_PARTY_NOTICES.md`.

- [ ] **Step 4: Run the test and manifest checks**

Run: `npx vitest run src/config.test.ts`

Expected: PASS.

Run: `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps --format-version 1`

Expected: package name `codex-agent-model-monitor` and no manifest error.

- [ ] **Step 5: Commit**

```bash
git add package.json src/config.ts src/config.test.ts src-tauri/Cargo.toml src-tauri/tauri.conf.json src-tauri/capabilities/default.json src-tauri/src/main.rs THIRD_PARTY_NOTICES.md
git commit -m "chore: establish Codex Agent Monitor product"
```

### Task 2: Define observations and deterministic reconciliation

**Files:**
- Create: `src-tauri/src/monitor/mod.rs`
- Create: `src-tauri/src/monitor/model.rs`
- Create: `src-tauri/src/monitor/reconcile.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `DbThreadFact`, `SpawnFact`, and `RolloutFact` metadata structures.
- Produces: `reconcile(input: ReconcileInput, now_ms: i64) -> MonitorSnapshot`.

- [ ] **Step 1: Write reconciliation tests**

Cover these exact cases inside `reconcile.rs`:

```rust
#[test]
fn rollout_model_wins_and_drift_is_visible() {
    let snapshot = reconcile(fixture("gpt-5.6-sol", "gpt-5.6-terra"), 10_000);
    let child = snapshot.agents.iter().find(|a| a.is_subagent).unwrap();
    assert_eq!(child.requested_model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(child.effective_model.as_deref(), Some("gpt-5.6-terra"));
    assert_eq!(child.model_source, ModelSource::TurnContext);
    assert!(child.model_drift);
}
```

Also assert `task_started -> running`, `task_complete -> idle`, newer interruption -> interrupted, missing child context -> starting, missing required identity -> tracking-error, and nested depth calculation.

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml monitor::reconcile`

Expected: FAIL because the monitor domain does not exist.

- [ ] **Step 3: Implement minimal domain types and reconciliation**

Use serialized enums with kebab-case values:

```rust
pub enum AgentStatus { Starting, Running, Idle, Interrupted, TrackingError }
pub enum ModelSource { TurnContext, StateDatabase, RequestedOnly, Unknown }
pub struct AgentObservation {
    pub thread_id: String,
    pub parent_thread_id: Option<String>,
    pub agent_path: Option<String>,
    pub display_name: String,
    pub project: Option<String>,
    pub originator: Option<String>,
    pub requested_model: Option<String>,
    pub effective_model: Option<String>,
    pub model_source: ModelSource,
    pub reasoning_effort: Option<String>,
    pub status: AgentStatus,
    pub model_drift: bool,
    pub is_subagent: bool,
    pub depth: u32,
    pub started_at_ms: Option<i64>,
    pub updated_at_ms: Option<i64>,
}
```

Keep internal rollout paths and full cwd values out of `AgentObservation`.

- [ ] **Step 4: Run reconciliation tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml monitor::reconcile`

Expected: all reconciliation tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/monitor
git commit -m "feat: model agent observations and reconciliation"
```

### Task 3: Read Codex state through a read-only projection

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/monitor/sqlite_source.rs`

**Interfaces:**
- Produces: `read_state_db(codex_home: &Path) -> SourceResult<StateFacts>`.
- `StateFacts` contains safe display facts plus internal rollout paths consumed only by the backend.

- [ ] **Step 1: Write temporary-database tests**

Create a minimal fixture with `threads` and `thread_spawn_edges`, then assert the projection includes a root and child, returns model/effort, reduces `C:\secret\project` to `project`, and cannot execute a write on the same connection.

```rust
assert_eq!(facts.threads[1].project.as_deref(), Some("project"));
assert_eq!(facts.edges[0].child_thread_id, "child");
assert!(facts.open_mode.is_read_only());
```

- [ ] **Step 2: Verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sqlite_source`

Expected: FAIL because `read_state_db` is missing.

- [ ] **Step 3: Implement read-only SQLite access**

Add `rusqlite` with bundled SQLite and open with:

```rust
OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX
```

Query only `id`, `rollout_path`, `source`, `model_provider`, `cwd`, `title`, `agent_nickname`, `agent_role`, `model`, `reasoning_effort`, `agent_path`, `created_at_ms`, and `updated_at_ms`, plus parent/child IDs. Detect missing tables or columns and return a sanitized degraded-health category rather than SQL text containing local paths.

- [ ] **Step 4: Run database tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml sqlite_source`

Expected: PASS, including the write-denial assertion.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/monitor/sqlite_source.rs
git commit -m "feat: project Codex state in read-only mode"
```

### Task 4: Extract only whitelisted rollout metadata

**Files:**
- Create: `src-tauri/src/monitor/rollout_source.rs`

**Interfaces:**
- Produces: `RolloutIndex::refresh(&mut self, facts: &StateFacts) -> SourceResult<Vec<RolloutFact>>`.
- Guarantees: public facts contain no raw line, prompt, arguments, output, or path.

- [ ] **Step 1: Write privacy and parsing tests**

Use synthetic lines containing canary secrets in messages and tool output. Assert the parser returns no fact and its debug representation never contains the canary. Add positive cases for `turn_context`, task boundaries, `sub_agent_activity`, and correlated `spawn_agent` call/output.

```rust
let rejected = parse_line(r#"{"type":"response_item","payload":{"type":"message","content":"CANARY_SECRET"}}"#);
assert!(rejected.is_none());
assert!(!format!("{rejected:?}").contains("CANARY_SECRET"));
```

- [ ] **Step 2: Verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml rollout_source`

Expected: FAIL because the whitelist parser is absent.

- [ ] **Step 3: Implement envelope filtering and incremental cursors**

Reject lines before structured parsing unless they contain one of the permitted record/type markers. Parse permitted payloads into dedicated structs, never `serde_json::Value` returned beyond the function. Correlate a spawn output only when its call ID was previously registered as `spawn_agent`; retain only child ID, nickname, requested model, requested effort, task name, and timestamp.

Track `(length, modified, offset)` per file. On growth, seek to the prior offset; on truncation, reset to zero. Bound a single refresh to 32 MiB per file and report `backlog` health when more remains.

- [ ] **Step 4: Run whitelist tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml rollout_source`

Expected: PASS and all canary assertions remain clean.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/monitor/rollout_source.rs
git commit -m "feat: observe rollout metadata through a strict whitelist"
```

### Task 5: Add the live runtime and Tauri contract

**Files:**
- Create: `src-tauri/src/monitor/runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `shared/monitor-types.ts`
- Create: `src/lib/monitorApi.ts`
- Create: `src/lib/monitorApi.test.ts`

**Interfaces:**
- Produces Tauri commands `get_monitor_snapshot`, `refresh_monitor`, `get_monitor_settings`, and `set_codex_home`.
- Emits `monitor://snapshot` with `MonitorSnapshot` every time a reconciled snapshot materially changes.

- [ ] **Step 1: Write contract tests**

Assert TypeScript parses a fixture with `effective_model`, `requested_model`, `model_source`, `status`, and health fields, and that unexpected content fields are discarded by the explicit `toMonitorSnapshot` mapper.

- [ ] **Step 2: Verify frontend contract failure**

Run: `npx vitest run src/lib/monitorApi.test.ts`

Expected: FAIL because the adapter is missing.

- [ ] **Step 3: Implement the runtime**

Store the last snapshot behind `Arc<RwLock<_>>`. In Tauri setup, spawn a one-second refresh loop. Emit only when the serialized safe snapshot hash changes. Validate a custom home by requiring a directory plus either `state_5.sqlite` or `sessions`; persist the chosen path in the app config directory, never in Codex files.

Map backend errors to `{ code, message }` where message contains no full path or SQL statement.

- [ ] **Step 4: Implement and test the TypeScript adapter**

Use `invoke` for initial load and `listen<MonitorSnapshot>` for updates. Expose:

```ts
export interface MonitorApi {
  getSnapshot(): Promise<MonitorSnapshot>;
  refresh(): Promise<MonitorSnapshot>;
  subscribe(handler: (snapshot: MonitorSnapshot) => void): Promise<() => void>;
}
```

Run: `npx vitest run src/lib/monitorApi.test.ts`

Expected: PASS.

- [ ] **Step 5: Run backend tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml monitor`

Expected: PASS.

```bash
git add src-tauri/src/lib.rs src-tauri/src/monitor/runtime.rs shared/monitor-types.ts src/lib/monitorApi.ts src/lib/monitorApi.test.ts
git commit -m "feat: stream sanitized monitor snapshots"
```

### Task 6: Build the metadata-only agent window

**Files:**
- Create: `src/hooks/useMonitor.ts`
- Create: `src/components/AgentTree.tsx`
- Create: `src/components/AgentRow.tsx`
- Create: `src/components/FilterBar.tsx`
- Create: `src/components/HealthStrip.tsx`
- Create: `src/components/SettingsDialog.tsx`
- Create: `src/components/AgentTree.test.tsx`
- Create: `src/components/FilterBar.test.tsx`
- Modify: `src/App.tsx`
- Replace: `src/styles/global.css`

**Interfaces:**
- Consumes: `MonitorApi` and `MonitorSnapshot`.
- Produces: accessible root/child presentation and settings actions with no transcript route.

- [ ] **Step 1: Write UI tests**

Render a root with two children and assert:

```ts
expect(screen.getByText("gpt-5.6-terra")).toBeInTheDocument();
expect(screen.getByText("requested gpt-5.6-sol")).toBeInTheDocument();
expect(screen.getByLabelText("Model drift")).toBeInTheDocument();
expect(screen.queryByText("CANARY_SECRET")).not.toBeInTheDocument();
```

Test active/all filters, unknown model, tracking-error, nested descendants, keyboard-expand buttons, and empty state.

- [ ] **Step 2: Verify failure**

Run: `npx vitest run src/components/AgentTree.test.tsx src/components/FilterBar.test.tsx`

Expected: FAIL because the monitor components do not exist.

- [ ] **Step 3: Implement the window**

Use semantic buttons and lists, a visible focus ring, text plus color for status, and `aria-label` for drift/health. Show the effective model as the primary badge. Show requested model only when effective is pending or different. Display only short thread IDs and project basename.

Replace the old picker/list/detail state machine. Remove imports that make transcript, tool-output, Markdown, syntax-highlighter, and direct filesystem functionality reachable.

- [ ] **Step 4: Run UI tests and type check**

Run: `npx vitest run src/components/AgentTree.test.tsx src/components/FilterBar.test.tsx`

Expected: PASS.

Run: `npx tsc --noEmit`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/App.tsx src/hooks/useMonitor.ts src/components/AgentTree.tsx src/components/AgentRow.tsx src/components/FilterBar.tsx src/components/HealthStrip.tsx src/components/SettingsDialog.tsx src/components/*.test.tsx src/styles/global.css
git commit -m "feat: present live subagent model hierarchy"
```

### Task 7: Enforce privacy with integration fixtures

**Files:**
- Create: `src-tauri/tests/monitor_fixture.rs`
- Create: `src-tauri/tests/fixtures/rollout-root.jsonl`
- Create: `src-tauri/tests/fixtures/rollout-child.jsonl`
- Modify: `src-tauri/src/monitor/runtime.rs`

**Interfaces:**
- Consumes: public `monitor::snapshot_for_home(&Path)` test entry point.
- Produces: an end-to-end sanitized `MonitorSnapshot` from a synthetic Codex home.

- [ ] **Step 1: Add a failing end-to-end fixture test**

The fixture includes a root spawning a child, child `turn_context` model/effort, running and completed turns, and canaries in user messages, tool arguments, and tool output. Serialize the snapshot and assert every canary and full fixture path is absent.

- [ ] **Step 2: Run and observe failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test monitor_fixture`

Expected: FAIL until the public fixture entry point and all reconciliation paths are complete.

- [ ] **Step 3: Complete missing integration seams**

Expose only the path-taking test function, keep internal paths private, and ensure an idle child can later transition back to running when a follow-up `task_started` record is appended.

- [ ] **Step 4: Run integration and full checks**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test monitor_fixture`

Expected: PASS.

Run: `npm run check`

Expected: PASS with zero lint, format, type, frontend-test, clippy, or Rust-test errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tests src-tauri/src/monitor/runtime.rs
git commit -m "test: verify metadata-only monitoring end to end"
```

### Task 8: Document, package, and smoke-test Windows delivery

**Files:**
- Replace: `README.md`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `package-lock.json`
- Remove from dependencies: Markdown, syntax-highlighter, frontend filesystem plugin, HTTP server packages no longer used.

**Interfaces:**
- Produces: Windows NSIS installer and release executable under `src-tauri/target/release/bundle/` and `src-tauri/target/release/`.

- [ ] **Step 1: Update documentation**

Document installation, the metadata-only contract, model precedence, lifecycle meanings, custom `CODEX_HOME`, degraded mode, uninstall, and Codex Trace attribution. Do not claim official OpenAI affiliation.

- [ ] **Step 2: Remove unreachable content-viewer dependencies**

Remove packages and Rust modules that provide Markdown rendering, syntax highlighting, frontend direct filesystem access, Axum web mode, and transcript HTTP APIs. Refresh lockfiles and ensure the generated bundle has no web-server startup.

- [ ] **Step 3: Run the release gate**

Run: `npm run check`

Expected: PASS.

Run: `npm run tauri build -- --bundles nsis`

Expected: release executable plus one NSIS installer for `Codex Agent Monitor`.

- [ ] **Step 4: Smoke-test the installed app**

Install silently into the current user profile, launch the installed executable hidden from an automation-visible console, wait for the main window process, verify it remains alive and does not modify hashes of `state_5.sqlite` or sampled rollout fixtures, then close it normally.

Expected: process starts, snapshot refresh succeeds, source hashes remain identical, and uninstall entry is present.

- [ ] **Step 5: Commit**

```bash
git add README.md package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src src-tauri/tauri.conf.json
git commit -m "release: package Codex Agent Monitor for Windows"
```

### Task 9: Build and deploy the Sites download page

**Files:**
- Create through Sites initializer: `site/.openai/hosting.json`
- Modify after initialization: `site/app/page.tsx`
- Modify after initialization: `site/app/layout.tsx`
- Modify after initialization: `site/app/globals.css`
- Copy validated artifact: `site/public/downloads/Codex-Agent-Monitor-Setup.exe`

**Interfaces:**
- Consumes: validated installer, version, privacy contract, and installation copy.
- Produces: deployed private Sites URL with a working installer link.

- [ ] **Step 1: Initialize the site once**

Run the Sites plugin initializer with the empty `site` directory as its target, retain its package manager and lockfile, start its development server, and open the exact local URL once.

- [ ] **Step 2: Replace the starter with the product page**

Create one responsive route with these exact sections: hero and download button, live fields shown by the desktop app, “requested versus effective model” explanation, metadata-only privacy boundary, installation steps, source/attribution, and version. Use CSS shapes and typography; no decorative generated imagery is required.

- [ ] **Step 3: Add the validated installer and metadata**

Copy the exact smoke-tested installer to `public/downloads/Codex-Agent-Monitor-Setup.exe`. Set the page title and description to `Codex Agent Monitor — See the model behind every Codex subagent` and remove all starter preview metadata/components.

- [ ] **Step 4: Build and publish**

Run: `npm run build`

Expected: successful Cloudflare Worker-compatible Sites build.

Use Sites hosting to create the site once, persist only `project_id` in `.openai/hosting.json`, save a version from the validated source, deploy privately, and poll until status is `succeeded`.

- [ ] **Step 5: Verify download handoff and commit**

Open the deployed URL in Codex, verify the primary download link targets `/downloads/Codex-Agent-Monitor-Setup.exe`, then commit the exact deployed site source without credentials or temporary archives.

```bash
git add site
git commit -m "deploy: publish Codex Agent Monitor download site"
```

### Task 10: Final acceptance and work record

**Files:**
- Update: `D:\Work_plan\README.md`

- [ ] **Step 1: Re-run immutable-source checks**

Hash the same Codex database and rollout samples before and after a live monitor refresh. Expected: hashes are identical.

- [ ] **Step 2: Record exact deliverables**

Capture the app version, Git commit, installer location, deployed Sites URL, test totals, release-build result, smoke-test result, and any optional follow-up.

- [ ] **Step 3: Append the verified work entry**

Use the required Asia/Shanghai `2026-07-18 星期六` format, avoid duplicate entries, and include no local secrets, thread IDs, raw paths from Codex metadata, or credentials.

- [ ] **Step 4: Final handoff**

Return the deployed Sites URL first, then the clickable local installer and project links, followed by a concise verification summary and confirmation that the work record was updated.
