# Codex Assistant Routing Recovery and Privacy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Smart Routing remain installed across unrelated Codex config edits, expose blocked preflight/toggle operations clearly, and remove prompt-derived titles and tool arguments from the monitor contract.

**Architecture:** Treat `config.toml` as a shared document and compare only Codex Assistant's exact owned keys while continuing to hash owned agent/skill files byte-for-byte. When restoring after unrelated config edits, surgically restore owned keys from the original backup and preserve every unrelated current key. At the metadata boundary, stop selecting thread titles and stop retaining spawn `task_name` tool arguments; derive labels only from project basename, system nickname/role, and an opaque thread-ID prefix.

**Tech Stack:** Rust 1.82, rusqlite read-only SQLite, toml_edit 0.22, Tauri 2, React 19, TypeScript 7, Vitest/Testing Library, native Codex MCP stdio, Playwright Interactive.

## Global Constraints

- Open the Codex state database read-only and retain the 250 ms busy timeout.
- Never retain, log, serialize, or display prompts, responses, reasoning, tool arguments, tool outputs, credentials, or full workspace paths.
- `config.toml` is shared state; only `agents.max_depth`, the four `agents.codex_assistant_*` owned keys, and `mcp_servers.codex_assistant_routing` owned keys belong to Codex Assistant.
- True edits to owned values must still fail closed as `config-conflict`; unrelated Codex/user edits must not disable routing.
- Root routing cannot be enabled before native preflight completes, and every blocked/failed receipt must remain visible until another explicit action.
- Public TDD seams: `CodexConfigService::inspect/install/restore`, `read_state_db`, `reconcile`, `useRouting`, and rendered `SmartRoutingPage`.

---

### Task 1: Compare and restore only owned config projections

**Files:**

- Modify: `src-tauri/src/codex_config/transaction.rs`
- Test: `src-tauri/tests/codex_config_transaction.rs`

**Interfaces:**

- Consumes: the current `config.toml`, the recorded pre-install bytes, and existing full-file manifest hashes.
- Produces: `config_ownership_matches_desired(&[u8]) -> Result<bool>`, `restore_owned_config(current, preimage) -> Result<Vec<u8>>`, and projection-aware `inspect`/`restore` behavior.

- [ ] **Step 1: Write the failing unrelated-edit inspection test**

```rust
#[test]
fn unrelated_config_changes_do_not_create_an_ownership_conflict() {
    let fixture = Fixture::new();
    let service = fixture.service("install");
    service.install().expect("install");
    fixture.append_config("\nmodel = \"gpt-5.6-sol\"\n");

    let inspected = service.inspect().expect("inspect");
    assert!(inspected.installed);
    assert!(!inspected.conflicts.contains(&"config.toml".to_owned()));
}

#[test]
fn owned_config_changes_still_report_a_conflict() {
    let fixture = Fixture::new();
    let service = fixture.service("install");
    service.install().expect("install");
    fixture.replace_config_value(
        "mcp_servers.codex_assistant_routing.enabled",
        "false",
    );
    assert!(service.inspect().expect("inspect").conflicts.contains(&"config.toml".to_owned()));
}
```

- [ ] **Step 2: Run the inspection tests and verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test codex_config_transaction config_changes -- --nocapture`

Expected: the unrelated-change test FAILS because `inspect()` compares the full file hash.

- [ ] **Step 3: Implement semantic ownership inspection**

```rust
fn config_ownership_matches_desired(&self, current: &[u8]) -> Result<bool> {
    Ok(self.merge_config(current)?.as_bytes() == current)
}
```

In `inspect()`, keep byte hashes for every owned asset, but special-case `config.toml` to call `config_ownership_matches_desired`. Missing owned keys return `false`; conflicting types/values remain a sanitized conflict. Do not update the existing manifest merely because unrelated keys changed.

- [ ] **Step 4: Run the inspection tests and verify green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test codex_config_transaction config_changes -- --nocapture`

Expected: PASS; unrelated edits are accepted and owned edits remain conflicts.

- [ ] **Step 5: Write the failing surgical restore tests**

```rust
#[test]
fn restore_removes_owned_keys_but_preserves_unrelated_changes() {
    let fixture = Fixture::with_config("model = \"gpt-5.6-terra\"\n");
    let service = fixture.service("install");
    service.install().expect("install");
    fixture.append_config("\napproval_policy = \"never\"\n");

    let receipt = service.restore().expect("restore");
    assert!(receipt.changed);
    assert!(receipt.conflicts.is_empty());
    let restored = fixture.config();
    assert!(restored.contains("approval_policy = \"never\""));
    assert!(!restored.contains("codex_assistant_routing"));
    assert!(!restored.contains("codex_assistant_sol"));
}

#[test]
fn restore_refuses_to_overwrite_a_changed_owned_value() {
    let fixture = Fixture::new();
    let service = fixture.service("install");
    service.install().expect("install");
    fixture.replace_config_value("mcp_servers.codex_assistant_routing.enabled", "false");
    let receipt = service.restore().expect("restore receipt");
    assert!(receipt.conflicts.contains(&"config.toml".to_owned()));
}
```

- [ ] **Step 6: Run the restore tests and verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test codex_config_transaction restore_ -- --nocapture`

Expected: the preserve-unrelated-change test FAILS because restore currently requires the full installed hash.

- [ ] **Step 7: Implement exact owned-key restoration**

```rust
fn restore_owned_config(&self, current_bytes: &[u8], preimage: &[u8]) -> Result<Vec<u8>> {
    if !self.config_ownership_matches_desired(current_bytes)? {
        return Err(ConfigError::new("Owned Codex setting conflicts with another owner"));
    }
    let mut current = parse_config(current_bytes)?;
    let original = parse_config(preimage)?;
    restore_item(&mut current, &original, &["agents", "max_depth"])?;
    for (name, _, _) in OWNED_AGENT_NAMES {
        restore_item(&mut current, &original, &["agents", name, "description"])?;
        restore_item(&mut current, &original, &["agents", name, "config_file"])?;
        remove_empty_table_if_originally_absent(&mut current, &original, &["agents", name]);
    }
    for key in ["command", "args", "enabled", "required", "enabled_tools"] {
        restore_item(
            &mut current,
            &original,
            &["mcp_servers", "codex_assistant_routing", key],
        )?;
    }
    remove_empty_table_if_originally_absent(
        &mut current,
        &original,
        &["mcp_servers", "codex_assistant_routing"],
    );
    Ok(render_with_original_line_endings(current, current_bytes))
}
```

Use the full preimage restore fast path when the full installed hash still matches. Otherwise, for `config.toml` only, require the owned projection to match the installed desired values, write the surgically restored document atomically, and report a conflict without writing when any owned value differs.

- [ ] **Step 8: Run transaction tests and verify green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test codex_config_transaction -- --nocapture`

Expected: PASS for rollback, idempotence, exact untouched restore, unrelated-change preservation, and true conflict protection.

- [ ] **Step 9: Commit the vertical slice**

```bash
git add src-tauri/src/codex_config/transaction.rs src-tauri/tests/codex_config_transaction.rs
git commit -m "fix: scope routing config ownership"
```

### Task 2: Remove prompt-derived labels and tool arguments at the source boundary

**Files:**

- Modify: `src-tauri/src/monitor/model.rs`
- Modify: `src-tauri/src/monitor/sqlite_source.rs`
- Modify: `src-tauri/src/monitor/rollout_source.rs`
- Modify: `src-tauri/src/monitor/reconcile.rs`
- Modify: `src-tauri/src/monitor/runtime.rs`
- Test: `src-tauri/tests/monitor_fixture.rs`
- Test: existing unit tests in the four monitor modules above

**Interfaces:**

- Consumes: read-only thread identity, lineage, model/effort, project basename, system nickname/role, timestamps, and status boundaries.
- Produces: privacy-safe `AgentObservation.display_name` with no `ThreadFact.title` or `SpawnFact.task_name` fields in memory or serialization.

- [ ] **Step 1: Write failing canary tests for DB titles and spawn tool arguments**

```rust
#[test]
fn state_source_never_collects_thread_titles() {
    let temporary = tempdir().expect("tempdir");
    create_fixture_with_title(temporary.path(), "CANARY_PRIVATE_PROMPT");
    let encoded = serde_json::to_string(&read_state_db(temporary.path()).expect("facts"))
        .unwrap_or_default();
    assert!(!encoded.contains("CANARY_PRIVATE_PROMPT"));
}

#[test]
fn reconciled_labels_ignore_titles_and_spawn_task_arguments() {
    let snapshot = reconcile(canary_input("CANARY_PRIVATE_PROMPT", "CANARY_TOOL_ARGUMENT"), 10_000);
    let encoded = serde_json::to_string(&snapshot).expect("snapshot");
    assert!(!encoded.contains("CANARY_PRIVATE_PROMPT"));
    assert!(!encoded.contains("CANARY_TOOL_ARGUMENT"));
    assert!(snapshot.agents.iter().all(|agent| agent.display_name.len() <= 80));
}
```

- [ ] **Step 2: Run monitor tests and verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml monitor -- --nocapture`

Expected: FAIL because the SQLite query selects `t.title`, `ThreadFact` stores it, and `display_name` prefers it; rollout parsing also retains `task_name`.

- [ ] **Step 3: Remove title and task-name collection**

Delete `ThreadFact.title` and `SpawnFact.task_name`. Remove `t.title` from the SQLite SELECT and update row indices. In rollout parsing, deserialize only model, effort, and child identity fields needed for metadata reconciliation; do not copy `task_name` from the `spawn_agent` arguments into facts.

- [ ] **Step 4: Derive bounded labels from allowed metadata**

```rust
fn display_name(thread: &ThreadFact) -> String {
    let opaque = thread.thread_id.chars().take(8).collect::<String>();
    if thread.parent_thread_id.is_none() {
        return thread
            .project
            .as_deref()
            .filter(|value| !value.is_empty())
            .map_or_else(|| format!("根任务 {opaque}"), |project| format!("{project} · {opaque}"));
    }
    thread
        .nickname
        .as_deref()
        .or(thread.role.as_deref())
        .filter(|value| !value.is_empty() && value.len() <= 48)
        .map_or_else(|| format!("子代理 {opaque}"), |label| format!("{label} · {opaque}"))
}
```

Do not use `agent_path` as display text because its components may originate from user-provided task labels. Continue exposing the project basename only, never the full workspace path.

- [ ] **Step 5: Run monitor and privacy tests and verify green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml monitor routing_state_privacy monitor_fixture -- --nocapture`

Expected: PASS; the canaries are absent from facts, snapshots, and serialized output while lineage/model/status behavior remains unchanged.

- [ ] **Step 6: Commit the vertical slice**

```bash
git add src-tauri/src/monitor/model.rs src-tauri/src/monitor/sqlite_source.rs src-tauri/src/monitor/rollout_source.rs src-tauri/src/monitor/reconcile.rs src-tauri/src/monitor/runtime.rs src-tauri/tests/monitor_fixture.rs
git commit -m "fix: remove prompt-derived monitor labels"
```

### Task 3: Surface config conflicts, preflight requirements, and blocked receipts

**Files:**

- Modify: `src/hooks/useRouting.ts`
- Modify: `src/hooks/useRouting.test.ts`
- Modify: `src/components/SmartRoutingPage.tsx`
- Modify: `src/components/SmartRoutingPage.test.tsx`
- Modify: `src/styles/global.css`

**Interfaces:**

- Consumes: `RoutingUiSnapshot.setup.installation_status/reason_codes/preflight_status` and `RoutingOperationReceipt`.
- Produces: sticky error messages, explicit conflict/preflight panels, and route controls enabled only when the setup state permits the requested mutation.

- [ ] **Step 1: Write failing hook tests for blocked toggle persistence**

```ts
it("surfaces a blocked root toggle and polling does not erase it", async () => {
  routingApi.setRootEnabled = vi.fn().mockResolvedValue({
    operation_id: "toggle-1",
    status: "blocked",
    reason_codes: ["preflight-required"],
    restart_required: false,
  });
  const { result } = renderHook(() => useRouting());
  await act(() => result.current.setRootEnabled(ROOT_ID, true));
  expect(result.current.error).toMatch(/先完成原生能力预检/);
  await act(() => vi.advanceTimersByTime(5_000));
  expect(result.current.error).toMatch(/先完成原生能力预检/);
});
```

- [ ] **Step 2: Run the hook test and verify red**

Run: `npx vitest run src/hooks/useRouting.test.ts`

Expected: FAIL because `setRootEnabled` ignores blocked receipts and `accept()` clears errors.

- [ ] **Step 3: Implement one receipt-to-message path and sticky errors**

```ts
const OPERATION_FAILURE_MESSAGES: Record<string, string> = {
  "config-conflict": "Smart Routing 自有配置已被修改；请修复安装后再继续。",
  "preflight-required": "请先完成当前根任务的原生能力预检，再启用 Smart Routing。",
  "routing-runtime-unavailable": "Smart Routing 本地运行时不可用；状态未被更改。",
  "cdp-unavailable": "当前 Codex 控制会话不可用；请先安全重启 Codex。",
};

function receiptError(receipt: RoutingOperationReceipt): string | null {
  return receipt.status === "applied" || receipt.status === "noop"
    ? null
    : (OPERATION_FAILURE_MESSAGES[receipt.reason_codes[0] ?? ""] ??
        "Smart Routing 操作未完成；已保留上一次验证状态。");
}
```

Set `error` from every mutation receipt after accepting the refreshed snapshot. Successful background snapshots update connection/snapshot state without clearing operation errors. Clear the error at the start of the next explicit mutation or a successful manual refresh.

- [ ] **Step 4: Run the hook test and verify green**

Run: `npx vitest run src/hooks/useRouting.test.ts`

Expected: PASS; the blocked reason is shown and survives background polling.

- [ ] **Step 5: Write failing page tests for conflict and preflight states**

```tsx
it("explains a true config conflict and disables route toggles", () => {
  mockRouting(conflictSnapshot);
  render(<SmartRoutingPage roots={roots} />);
  expect(screen.getByRole("heading", { name: "安装需要修复" })).toBeVisible();
  expect(screen.getByText(/自有配置项与当前值冲突/)).toBeVisible();
  expect(screen.getByRole("button", { name: /启用 .* Smart Routing/ })).toBeDisabled();
});

it("directs an installed root to preflight before enabling", () => {
  mockRouting(installedNotStartedSnapshot);
  render(<SmartRoutingPage roots={roots} />);
  expect(screen.getByRole("button", { name: "开始原生能力预检" })).toBeEnabled();
  expect(screen.getByRole("button", { name: /启用 .* Smart Routing/ })).toBeDisabled();
});
```

- [ ] **Step 6: Run the page tests and verify red**

Run: `npx vitest run src/components/SmartRoutingPage.test.tsx`

Expected: FAIL because conflict is rendered as generic `安装状态` and route controls remain enabled.

- [ ] **Step 7: Implement explicit setup-state UX using existing panels/buttons**

Render these exact headings:

```ts
const setupHeading =
  setup?.installation_status === "uninstalled"
    ? "尚未安装"
    : setup?.installation_status === "conflict"
      ? "安装需要修复"
      : setup?.installation_status === "restart-required"
        ? "等待 Codex 重启"
        : "Smart Routing 已安装";
```

For `conflict`, show `Smart Routing 自有配置项与当前值冲突；不会覆盖未知修改。` and a `修复 Smart Routing` button that invokes the existing `install()` transaction. Disable each root toggle unless installation is `installed`, restart is `not-required`, preflight is `complete`, CDP is `ready`, and no operation is active. Add a nearby status sentence explaining the first unmet prerequisite.

- [ ] **Step 8: Run page and hook tests and verify green**

Run: `npx vitest run src/hooks/useRouting.test.ts src/components/SmartRoutingPage.test.tsx`

Expected: PASS with explicit conflict/preflight guidance and no silent no-op controls.

- [ ] **Step 9: Commit the vertical slice**

```bash
git add src/hooks/useRouting.ts src/hooks/useRouting.test.ts src/components/SmartRoutingPage.tsx src/components/SmartRoutingPage.test.tsx src/styles/global.css
git commit -m "fix: explain blocked Smart Routing operations"
```

### Task 4: Reinstall, restart, and verify native Smart Routing end to end

**Files:**

- Modify: `package.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `CHANGELOG.md`
- Modify: `docs/superpowers/diagnostics/2026-07-19-codex-assistant-runtime-qa.md`
- Create: `outputs/assistant-smart-routing-ready.png`
- Create: `outputs/codex-smart-routing-control.png`

**Interfaces:**

- Consumes: the repaired owned config, enabled `codex_assistant_routing` MCP server, a fresh official Codex process, and one visible root task.
- Produces: installed Codex Assistant 0.7.1, a completed native preflight, an enabled root route, and visible routing control evidence.

- [ ] **Step 1: Bump the synchronized desktop version and changelog**

Set `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` to `0.7.1`. Add a `0.7.1` changelog entry describing projection-scoped config ownership, visible blocked-operation reasons, source-boundary privacy removal, truthful theme verification, and local-only theme catalog support.

- [ ] **Step 2: Run the full automated quality gate**

Run: `npm run check`

Expected: TypeScript, oxlint, oxfmt, Clippy, rustfmt, Vitest, and every Rust test PASS.

- [ ] **Step 3: Build and install the 0.7.1 Windows package**

Run: `npm run tauri build`

Expected: a signed/packageable current-user NSIS installer under `src-tauri/target/release/bundle/nsis/`. Install it over 0.7.0 and verify `D:\Software\Codex Agent Monitor\codex-assistant.exe` reports file version `0.7.1`.

- [ ] **Step 4: Verify MCP registration before restarting Codex**

Run:

```powershell
& "$env:APPDATA\npm\codex.cmd" mcp get codex_assistant_routing
```

Expected: enabled stdio command points to the installed 0.7.1 executable with argument `routing-mcp`, and enabled tools are exactly `routing_policy_get`, `routing_route_started`, `routing_quality_record`.

- [ ] **Step 5: Perform one authorized controlled restart and verify the host**

Use Codex Assistant's fresh single-use restart ticket. The user already authorized termination of the reported active native agents; create and confirm the ticket immediately so it does not expire. Verify exactly one official Codex root process remains, its CDP listener is loopback-only, and no Dream Skin injector/port 9335 is active.

- [ ] **Step 6: Complete native preflight on one visible root task**

In Smart Routing, choose the privacy-safe root label, start preflight, and follow the visible validation instruction in the official Codex UI. Verify the matrix reaches a terminal `eligible` or explicit `unavailable` state based on observed effective-model metadata; do not mark requested-only models eligible. Capture `outputs/assistant-smart-routing-ready.png` after preflight completes.

- [ ] **Step 7: Enable the root route and verify visible control binding**

Enable Smart Routing for that root. Assert the operation receipt is `applied|noop` with no reason codes, the route snapshot is `enabled`, the control status is `enabled` or `pending-next-turn` as appropriate, and the Codex composer contains the owned routing control bound to the same opaque conversation ID. Capture `outputs/codex-smart-routing-control.png`.

- [ ] **Step 8: Verify MCP availability in a fresh Codex task**

Create one fresh test task after the restart and inspect its available tool inventory. Expected: the three `codex_assistant_routing` metadata-only tools are present. Call `routing_policy_get` with the visible root conversation ID and verify the response contains route policy/eligibility metadata only and no prompt, response, reasoning, tool argument/output, credential, or full path fields.

- [ ] **Step 9: Record evidence and commit**

Append process IDs (non-sensitive), loopback port check, config inspection status, preflight result, route/control state, MCP tool inventory, privacy canary result, and screenshot paths to `docs/superpowers/diagnostics/2026-07-19-codex-assistant-runtime-qa.md`.

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json CHANGELOG.md docs/superpowers/diagnostics/2026-07-19-codex-assistant-runtime-qa.md outputs/assistant-smart-routing-ready.png outputs/codex-smart-routing-control.png
git commit -m "release: verify Codex Assistant 0.7.1"
```
