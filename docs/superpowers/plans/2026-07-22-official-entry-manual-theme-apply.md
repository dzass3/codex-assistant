# Official Entry Manual Theme Apply Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Codex Assistant 0.10.0 with one supported launch model: the official ChatGPT/Codex entry remains untouched, and a theme is applied only after the user explicitly opens Codex Assistant and clicks “应用主题”; theme selection persists, but a fully reopened official app requires another explicit apply.

**Architecture:** Keep the verified Store-package discovery, guarded restart, loopback CDP ownership, DOM compatibility checks, and device-only preference store. Remove the short-lived themed launcher and installer-created alternate shortcut. Replace the launcher-aware environment contract with a versioned on-demand contract shared by Rust and TypeScript, then make the UI state the manual-reapply boundary honestly. Upgrades clean only the exact legacy shortcut owned by prior Codex Assistant installers.

**Tech Stack:** Tauri 2, Rust 2021, React 19, TypeScript 7, Vitest, Rust integration tests, NSIS installer hooks, Next.js/Cloudflare website, PowerShell on Windows.

## Global Constraints

- Preserve the official Microsoft Store ChatGPT/Codex package, Start menu entry, AppUserModelID, data directory, SQLite database, user account, and official shortcuts byte-for-byte wherever Codex Assistant has no ownership.
- Do not create an alternate visible app, `Codex（主题版）` shortcut, startup-folder entry, `Run` registry value, scheduled task, tray process, watcher, supervisor, or automatic relaunch path.
- Persist only the selected theme preference and device-local imported assets. Do not claim that the visual theme persists after the official app is fully closed and reopened.
- A running official app without a verified theme session may be restarted only after explicit, current user confirmation. Continue to fail closed on active native work, ambiguous process identity, a changed process tree, residual descendants, unstable app-server identity, or CDP verification failure.
- Keep the recent SQLite safety fixes and their regression tests intact. Codex Assistant must never touch, move, replace, lock, or initialize the official application database.
- Keep background layers `pointer-events: none`; do not override semantic text colors, icon/SVG colors, focus indicators, input behavior, primary actions, or native layout ownership.
- Work in the existing dirty tree without reverting or overwriting unrelated user changes. Before each commit, review the staged diff. If the work is not in a task-owned isolated worktree, skip the commit step and report the exact reason.
- Do not install the generated NSIS package, terminate the active Codex window, or publish the website without a separate explicit runtime/deployment authorization.
- Historical plans remain historical evidence. Do not rewrite `docs/superpowers/plans/2026-07-21-theme-readiness-and-persistent-launch.md`.

## File Structure

### Remove

- `src-tauri/src/theme_launcher.rs` — delete the alternate-entry launcher implementation and its file lock.
- `src-tauri/tests/theme_launcher.rs` — delete tests for behavior that is no longer supported.

### Create

- `docs/adr/0005-on-demand-official-entry-theme-flow.md` — record the official-entry/manual-apply decision and migration consequences.
- `website/scripts/sync-installer-proof.mjs` — copy the built 0.10.0 installer into the website and update its exact byte count and SHA-256 proof deterministically.

### Modify

- `shared/theme-types.ts` — environment report contract version 2 without launcher state.
- `src-tauri/src/theme_environment.rs` — classify only platform, official package, process count, verified session, and saved preference.
- `src-tauri/tests/theme_environment.rs` — drive the new state machine test-first.
- `src-tauri/src/main.rs` and `src-tauri/src/lib.rs` — remove the `--launch-themed-codex` execution route and module export.
- `src-tauri/windows/installer-hooks.nsh` — stop creating the alternate shortcut and safely remove an exact legacy owned shortcut on upgrade/uninstall.
- `src-tauri/tests/product_identity.rs` — lock the no-alternate-entry/no-autorun invariant and the 0.10.0 identity.
- `src/lib/themeApi.ts` and `src/lib/themeApi.test.ts` — parse contract version 2 strictly and reject stale/extra launcher fields.
- `src/hooks/useTheme.ts` and `src/hooks/useTheme.test.ts` — retain explicit apply/restart behavior while consuming the new report.
- `src/components/ThemesPage.tsx` and `src/components/ThemesPage.test.tsx` — present “启动并应用”/“确认重启并应用” actions and the manual-reapply disclosure.
- `CONTEXT.md`, `docs/adr/0004-theme-only-product.md`, `README.md`, and `CHANGELOG.md` — align domain language and product claims.
- `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` — set release version 0.10.0.
- `website/app/page.tsx`, `website/tests/rendered-html.test.mjs`, `website/package.json`, `website/package-lock.json`, `website/README.md`, and `website/design-qa.md` — publish honest 0.10.0 behavior and locally verify the exact installer artifact.

---

## Task 1: Lock the on-demand environment contract with failing tests

**Files:**

- Modify: `src-tauri/tests/theme_environment.rs`
- Modify: `src/lib/themeApi.test.ts`
- Modify: `shared/theme-types.ts`
- Modify: `src-tauri/src/theme_environment.rs`
- Modify: `src/lib/themeApi.ts`

- [ ] **Step 1: Replace the Rust environment fixtures with the four supported states**

In `src-tauri/tests/theme_environment.rs`, remove `launcher_installed` from every fixture and assert these exact transitions:

```rust
use codex_assistant_lib::theme_environment::{
    classify_environment, ThemeEnvironmentProbe, ThemeEnvironmentStatus, ThemeNextAction,
};

fn probe() -> ThemeEnvironmentProbe {
    ThemeEnvironmentProbe {
        platform_supported: true,
        package_version: Some("26.715.8383.0".into()),
        verified_process_count: 1,
        session_reachable: true,
        selected_theme_id: Some("aurora-grid".into()),
    }
}

#[test]
fn reports_contract_two_without_a_launcher_requirement() {
    let report = classify_environment(probe());
    assert_eq!(report.contract_version, 2);
    assert_eq!(report.status, ThemeEnvironmentStatus::Ready);
    assert_eq!(report.next_action, ThemeNextAction::ApplyNow);
    assert!(report.can_apply_now);
    assert_eq!(report.checks.len(), 5);
}

#[test]
fn stale_session_requires_explicit_restart_confirmation() {
    let report = classify_environment(ThemeEnvironmentProbe {
        session_reachable: false,
        ..probe()
    });
    assert_eq!(report.status, ThemeEnvironmentStatus::RestartRequired);
    assert_eq!(report.next_action, ThemeNextAction::ConfirmRestart);
    assert!(!report.can_apply_now);
}

#[test]
fn stopped_official_app_can_be_launched_only_by_the_current_user_action() {
    let report = classify_environment(ThemeEnvironmentProbe {
        verified_process_count: 0,
        session_reachable: false,
        ..probe()
    });
    assert_eq!(report.status, ThemeEnvironmentStatus::CodexNotRunning);
    assert_eq!(report.next_action, ThemeNextAction::LaunchCodexForTheme);
    assert!(!report.can_apply_now);
}

#[test]
fn unsupported_and_ambiguous_environments_fail_closed() {
    let missing = classify_environment(ThemeEnvironmentProbe {
        package_version: None,
        verified_process_count: 0,
        session_reachable: false,
        selected_theme_id: None,
        ..probe()
    });
    assert_eq!(missing.status, ThemeEnvironmentStatus::Unsupported);
    assert_eq!(missing.next_action, ThemeNextAction::InstallCodex);

    let ambiguous = classify_environment(ThemeEnvironmentProbe {
        verified_process_count: 2,
        session_reachable: false,
        ..probe()
    });
    assert_eq!(ambiguous.status, ThemeEnvironmentStatus::Unsupported);
    assert_eq!(ambiguous.next_action, ThemeNextAction::CloseExtraWindows);
}
```

- [ ] **Step 2: Run the Rust test and verify it fails for the removed launcher contract**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test theme_environment
```

Expected: compilation failures mention missing `ConfirmRestart` and `LaunchCodexForTheme`, and the existing probe still requires `launcher_installed`.

- [ ] **Step 3: Add a strict TypeScript contract-v2 parser test**

In `src/lib/themeApi.test.ts`, define the valid report without a launcher field and add stale-contract rejection:

```ts
const environmentV2 = {
  contract_version: 2,
  status: "restart-required",
  checks: [
    { code: "supported-windows", state: "pass" },
    { code: "official-store-codex", state: "pass" },
    { code: "single-codex-window", state: "pass" },
    { code: "verified-theme-session", state: "action" },
    { code: "saved-theme", state: "pass" },
  ],
  codex_version: "26.715.8383.0",
  verified_process_count: 1,
  session_reachable: false,
  selected_theme_id: "aurora-grid",
  next_action: "confirm-restart",
  can_apply_now: false,
} as const;

it("accepts only the on-demand environment contract", async () => {
  invokeMock.mockResolvedValueOnce(environmentV2);
  await expect(themeApi.getEnvironment()).resolves.toEqual(environmentV2);

  invokeMock.mockResolvedValueOnce({
    ...environmentV2,
    contract_version: 1,
    launcher_installed: true,
  });
  await expect(themeApi.getEnvironment()).resolves.toBeNull();
});
```

- [ ] **Step 4: Run the focused TypeScript test and verify the red state**

Run:

```powershell
npx vitest run src/lib/themeApi.test.ts
```

Expected: the version-2 report is rejected because the parser still expects contract version 1 and `launcher_installed`.

- [ ] **Step 5: Replace the shared environment types with contract version 2**

In `shared/theme-types.ts`, use this exact public shape:

```ts
export type ThemeEnvironmentStatus =
  "ready" | "codex-not-running" | "restart-required" | "unsupported";

export type ThemeNextAction =
  | "apply-now"
  | "launch-codex-for-theme"
  | "confirm-restart"
  | "install-codex"
  | "close-extra-windows"
  | "none";

export type ThemeEnvironmentCheckCode =
  | "supported-windows"
  | "official-store-codex"
  | "single-codex-window"
  | "verified-theme-session"
  | "saved-theme";

export interface ThemeEnvironmentReport {
  contract_version: 2;
  status: ThemeEnvironmentStatus;
  checks: ThemeEnvironmentCheck[];
  codex_version: string | null;
  verified_process_count: number;
  session_reachable: boolean;
  selected_theme_id: string | null;
  next_action: ThemeNextAction;
  can_apply_now: boolean;
}
```

- [ ] **Step 6: Implement the matching Rust classifier**

In `src-tauri/src/theme_environment.rs`, remove `THEMED_LAUNCHER_FILE_NAME`, `SetupRequired`, `InstallLauncher`, `ThemedLauncher`, `launcher_installed`, and `themed_launcher_path`. Define the new actions and classifier as follows:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeNextAction {
    ApplyNow,
    LaunchCodexForTheme,
    ConfirmRestart,
    InstallCodex,
    CloseExtraWindows,
    None,
}

pub fn classify_environment(probe: ThemeEnvironmentProbe) -> ThemeEnvironmentReport {
    let (status, next_action, can_apply_now) = if !probe.platform_supported {
        (ThemeEnvironmentStatus::Unsupported, ThemeNextAction::None, false)
    } else if probe.package_version.is_none() {
        (
            ThemeEnvironmentStatus::Unsupported,
            ThemeNextAction::InstallCodex,
            false,
        )
    } else if probe.verified_process_count > 1 {
        (
            ThemeEnvironmentStatus::Unsupported,
            ThemeNextAction::CloseExtraWindows,
            false,
        )
    } else if probe.verified_process_count == 0 {
        (
            ThemeEnvironmentStatus::CodexNotRunning,
            ThemeNextAction::LaunchCodexForTheme,
            false,
        )
    } else if !probe.session_reachable {
        (
            ThemeEnvironmentStatus::RestartRequired,
            ThemeNextAction::ConfirmRestart,
            false,
        )
    } else {
        (
            ThemeEnvironmentStatus::Ready,
            ThemeNextAction::ApplyNow,
            true,
        )
    };

    ThemeEnvironmentReport {
        contract_version: 2,
        status,
        checks: checks(&probe),
        codex_version: probe.package_version,
        verified_process_count: probe.verified_process_count,
        session_reachable: probe.session_reachable,
        selected_theme_id: probe.selected_theme_id,
        next_action,
        can_apply_now,
    }
}
```

Keep only five checks in this order: `SupportedWindows`, `OfficialStoreCodex`, `SingleCodexWindow`, `VerifiedThemeSession`, `SavedTheme`.

- [ ] **Step 7: Update the strict frontend parser**

In `src/lib/themeApi.ts`:

- Replace the status/action/check sets with the exact values from `shared/theme-types.ts`.
- Require `contract_version === 2`.
- Require exactly these report keys:

```ts
[
  "contract_version",
  "status",
  "checks",
  "codex_version",
  "verified_process_count",
  "session_reachable",
  "selected_theme_id",
  "next_action",
  "can_apply_now",
];
```

- Delete all reads and returned values for `launcher_installed`.

- [ ] **Step 8: Run focused contract tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test theme_environment
npx vitest run src/lib/themeApi.test.ts
```

Expected: both commands exit 0; the Rust report is version 2 and the TypeScript parser rejects the old launcher-aware report.

- [ ] **Step 9: Commit the contract slice if the execution is isolated**

```powershell
git add shared/theme-types.ts src-tauri/src/theme_environment.rs src-tauri/tests/theme_environment.rs src/lib/themeApi.ts src/lib/themeApi.test.ts
git diff --cached --check
git commit -m "refactor: define on-demand theme environment contract"
```

Expected: the staged diff contains only the contract slice. In the current shared dirty worktree, skip this commit rather than capturing earlier unrelated edits.

---

## Task 2: Remove the alternate launcher and retire its installer shortcut safely

**Files:**

- Modify: `src-tauri/tests/product_identity.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Delete: `src-tauri/src/theme_launcher.rs`
- Delete: `src-tauri/tests/theme_launcher.rs`
- Modify: `src-tauri/windows/installer-hooks.nsh`

- [ ] **Step 1: Write the product-identity regression before deleting code**

Add `LIB_RS` and replace the launcher test in `src-tauri/tests/product_identity.rs`:

```rust
const LIB_RS: &str = include_str!("../src/lib.rs");

#[test]
fn installer_exposes_no_alternate_codex_entry_or_background_start_path() {
    assert!(!MAIN_RS.contains("--launch-themed-codex"));
    assert!(!LIB_RS.contains("theme_launcher"));
    assert!(!NSIS_HOOK.contains(
        "CreateShortCut \"$SMPROGRAMS\\${RETIRED_THEMED_CODEX_SHORTCUT}\""
    ));
    assert!(NSIS_HOOK.contains(
        "!insertmacro IsShortcutTarget \"$SMPROGRAMS\\${RETIRED_THEMED_CODEX_SHORTCUT}\" \"$INSTDIR\\${MAINBINARYNAME}.exe\""
    ));
    assert!(!NSIS_HOOK.contains("$SMSTARTUP"));
    assert!(!NSIS_HOOK.contains("CurrentVersion\\Run"));
    assert!(!NSIS_HOOK.to_ascii_lowercase().contains("schtasks"));
}
```

- [ ] **Step 2: Run the identity test and verify the red state**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test product_identity installer_exposes_no_alternate_codex_entry_or_background_start_path
```

Expected: failure because `main.rs`, `lib.rs`, and the NSIS POSTINSTALL hook still contain the themed launcher.

- [ ] **Step 3: Remove the launcher execution path**

Make `src-tauri/src/main.rs` contain only the normal application startup:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Codex Assistant terminated unexpectedly: {info}");
    }));
    codex_assistant_lib::run();
}
```

Delete `pub mod theme_launcher;` from `src-tauri/src/lib.rs`, then delete:

```text
src-tauri/src/theme_launcher.rs
src-tauri/tests/theme_launcher.rs
```

Run `rg -n "theme_launcher|launch-themed-codex" src-tauri/src src-tauri/tests` and require no matches.

- [ ] **Step 4: Replace shortcut creation with exact legacy cleanup**

In `src-tauri/windows/installer-hooks.nsh` rename the constant:

```nsis
!define RETIRED_THEMED_CODEX_SHORTCUT "Codex（主题版）.lnk"
```

At `legacy_post_done`, replace `CreateShortCut` with target-verified cleanup:

```nsis
legacy_post_done:
  ; Codex Assistant 0.10.0 no longer owns an alternate Codex entry. Remove only
  ; the exact retired shortcut when it still targets this installed binary.
  !insertmacro IsShortcutTarget "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}"
    Delete "$SMPROGRAMS\${RETIRED_THEMED_CODEX_SHORTCUT}"
  ${EndIf}
!macroend
```

Use the same target-verified block in `NSIS_HOOK_PREUNINSTALL`. Do not delete by filename alone, and do not enumerate or modify official ChatGPT/Codex shortcuts.

- [ ] **Step 5: Run launcher absence and installer-safety tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test product_identity
cargo test --manifest-path src-tauri/Cargo.toml --test windows_package_identity
rg -n "CreateShortCut.*Codex（主题版）|--launch-themed-codex|theme_launcher" src-tauri src shared
```

Expected: both test binaries pass; the `rg` command returns no implementation matches. The retired filename may remain only in the target-verified cleanup constant and its assertions.

- [ ] **Step 6: Preserve SQLite/process-tree safety regressions explicitly**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test windows_package_identity safe_restart_requires_every_original_descendant_to_exit_before_activation
cargo test --manifest-path src-tauri/Cargo.toml --test windows_package_identity theme_session_requires_a_stable_direct_official_app_server
```

Expected: both pass. No code in `src-tauri/src/control_layer/windows_package.rs` is weakened to compensate for removal of the launcher.

- [ ] **Step 7: Commit the launcher-removal slice if isolated**

```powershell
git add src-tauri/src/main.rs src-tauri/src/lib.rs src-tauri/src/theme_launcher.rs src-tauri/tests/theme_launcher.rs src-tauri/windows/installer-hooks.nsh src-tauri/tests/product_identity.rs
git diff --cached --check
git commit -m "refactor: remove alternate themed Codex launcher"
```

Expected: only launcher removal and safe retired-shortcut cleanup are staged. Skip in a shared dirty worktree.

---

## Task 3: Make the desktop UI truthful and explicitly user-driven

**Files:**

- Modify: `src/components/ThemesPage.test.tsx`
- Modify: `src/hooks/useTheme.test.ts`
- Modify: `src/components/ThemesPage.tsx`
- Modify: `src/hooks/useTheme.ts`

- [ ] **Step 1: Update fixtures to contract version 2**

In both frontend test files, remove `launcher_installed`, remove `themed-launcher` checks, set `contract_version: 2`, and use `confirm-restart` or `launch-codex-for-theme` as appropriate.

- [ ] **Step 2: Add user-visible behavior tests before changing copy**

In `src/components/ThemesPage.test.tsx`, add these assertions:

```tsx
it("states that a normal full reopen requires another explicit apply", async () => {
  render(<ThemesPage />);
  expect(
    await screen.findByText(
      "主题选择会保留；完全关闭并从官方入口重新打开 ChatGPT/Codex 后，需要回到这里再次点击“应用主题”。",
    ),
  ).toBeVisible();
  expect(screen.queryByText(/Codex（主题版）/)).not.toBeInTheDocument();
});

it("labels an unmanaged running Codex as an explicit restart", async () => {
  environment = {
    ...environment,
    status: "restart-required",
    session_reachable: false,
    next_action: "confirm-restart",
    can_apply_now: false,
  };
  render(<ThemesPage />);
  expect(await screen.findByRole("button", { name: "确认重启并应用" })).toBeEnabled();
});

it("labels a stopped official app as a one-shot user launch", async () => {
  environment = {
    ...environment,
    status: "codex-not-running",
    verified_process_count: 0,
    session_reachable: false,
    next_action: "launch-codex-for-theme",
    can_apply_now: false,
  };
  render(<ThemesPage />);
  expect(await screen.findByRole("button", { name: "启动并应用主题" })).toBeEnabled();
});
```

If the file uses an API mock rather than a mutable `environment` binding, apply the exact state changes to that existing mock before rendering; do not duplicate the mock transport.

- [ ] **Step 3: Run the component tests and verify they fail on the old launcher copy**

```powershell
npx vitest run src/components/ThemesPage.test.tsx src/hooks/useTheme.test.ts
```

Expected: failures mention the missing manual-reapply sentence and old button labels.

- [ ] **Step 4: Replace launcher-aware labels and guidance**

In `src/components/ThemesPage.tsx`, compute the primary session label as:

```ts
const sessionActionLabel =
  nextAction === "confirm-restart"
    ? "确认重启并应用"
    : nextAction === "launch-codex-for-theme"
      ? "启动并应用主题"
      : paused
        ? "恢复主题会话"
        : "启动主题会话";
```

Replace the conditional description with these exact meanings:

```tsx
<p>
  {nextAction === "confirm-restart"
    ? "当前官方 ChatGPT/Codex 没有经过验证的主题会话。只有你确认后，Codex Assistant 才会关闭并重启官方应用；它不会在后台自动重启。"
    : nextAction === "launch-codex-for-theme"
      ? selectedThemeId
        ? "官方 ChatGPT/Codex 当前未运行。点击后会按你的这一次操作启动官方应用并应用已保存主题。"
        : "官方 ChatGPT/Codex 当前未运行。先选择主题，再由你点击启动并应用。"
      : paused
        ? "主题选择已保留，但当前没有经过验证的控制会话。重新应用时会再次验证界面可用性。"
        : "首次应用可能需要你确认重启官方 ChatGPT/Codex，以建立仅绑定本机、当前 Windows 用户和官方进程的主题会话。"}
</p>
```

Delete `themed-launcher` from `CHECK_LABELS`, delete the conditional persistence paragraph that reads `report.launcher_installed`, and render this disclosure unconditionally inside `ThemeEnvironmentPanel`:

```tsx
<p className="theme-environment-persistence">
  主题选择会保留；完全关闭并从官方入口重新打开 ChatGPT/Codex 后，需要回到这里再次点击“应用主题”。
</p>
```

Update `environmentGuidance`:

```ts
case "confirm-restart":
  return "当前官方应用无法在运行后补加受验证的本机主题端口；点击应用后会先显示重启影响并等待你确认。";
case "launch-codex-for-theme":
  return report.selected_theme_id
    ? "官方应用未运行。点击后会启动官方 ChatGPT/Codex 并应用已保存主题。"
    : "官方应用未运行。选择一套主题后再点击应用。";
```

Remove the `install-launcher` case and every claim that a shortcut automatically restores a theme.

- [ ] **Step 5: Preserve explicit confirmation behavior in the hook**

Do not add any mount effect, timer, process watcher, or call to `startSession` based solely on environment state. Keep calls user-triggered:

- `themes.activate(themeId)` originates from an “应用主题” click.
- `themes.startSession()` originates from the session action button.
- `active-work` opens `ForceRestartDialog` and requires a second explicit confirmation.
- Any non-active-work blocking or failure remains visible and fail-closed.

Add an assertion to `src/hooks/useTheme.test.ts` that advancing the 5-second polling interval refreshes state but never calls `start_theme_session`, `activate_theme`, or `confirm_force_restart`.

- [ ] **Step 6: Run focused UI tests**

```powershell
npx vitest run src/components/ThemesPage.test.tsx src/hooks/useTheme.test.ts src/App.test.tsx
```

Expected: all pass; the rendered UI contains the manual-reapply disclosure, contains no alternate-entry label, and performs no theme mutation during polling.

- [ ] **Step 7: Commit the desktop UX slice if isolated**

```powershell
git add src/components/ThemesPage.tsx src/components/ThemesPage.test.tsx src/hooks/useTheme.ts src/hooks/useTheme.test.ts
git diff --cached --check
git commit -m "feat: require explicit theme apply after official app restart"
```

Expected: only desktop UX and its tests are staged. Skip in a shared dirty worktree.

---

## Task 4: Record the decision and remove contradictory product claims

**Files:**

- Create: `docs/adr/0005-on-demand-official-entry-theme-flow.md`
- Modify: `docs/adr/0004-theme-only-product.md`
- Modify: `CONTEXT.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add ADR 0005 with the complete accepted decision**

Create `docs/adr/0005-on-demand-official-entry-theme-flow.md`:

```markdown
# ADR 0005: On-demand themes through the official ChatGPT/Codex entry

- Status: Accepted
- Date: 2026-07-22

## Context

The alternate `Codex（主题版）` entry made theme restoration appear automatic, but it changed the user's launch model and created a second visible application path. The accepted product boundary is stricter: the official Microsoft Store ChatGPT/Codex entry remains the only Codex entry, Codex Assistant does not remain resident, and the user decides both when Codex starts and when a theme is applied.

## Decision

Codex Assistant stores the selected theme preference but applies a theme only after an explicit action in Codex Assistant. If the official app is stopped, that action may launch the official AppUserModelID once and apply the selection. If the official app is already running without a verified theme session, Codex Assistant presents the restart impact and waits for explicit confirmation before a guarded restart.

The installer does not create an alternate Codex shortcut, startup entry, tray process, watcher, supervisor, scheduled task, or `Run` value. Upgrades remove only the retired `Codex（主题版）` shortcut when its target is the installed Codex Assistant binary. Official package files, shortcuts, application data, and SQLite databases are outside Codex Assistant ownership.

## Consequences

- Theme selection survives Codex Assistant and Windows restarts.
- The applied visual theme does not automatically survive a full close and ordinary reopen of official ChatGPT/Codex.
- After an ordinary reopen, the user returns to Codex Assistant and clicks `应用主题` again.
- Switching themes inside a currently verified session does not require another Codex restart.
- This decision supersedes ADR 0004 only where ADR 0004 specifies `Codex（主题版）` or automatic reapplication through that entry.
```

- [ ] **Step 2: Mark the superseded portion of ADR 0004**

Change its status line to:

```markdown
- Status: Accepted; launch and persistence sections superseded by ADR 0005
```

Add one sentence under the old shortcut decision: “This launch mechanism is retained as historical context and is not shipped from version 0.10.0 onward.” Do not rewrite the historical rationale.

- [ ] **Step 3: Align the domain model and README**

In `CONTEXT.md`:

- Remove the “Themed Codex entry” term.
- Add “Manual reapply boundary”: selected preference persists; applied CSS belongs to the current verified session and is re-created only by a later explicit apply.
- Replace the launcher persistence invariant with: “Codex starts only through the official entry or a current explicit Codex Assistant action; a normal full reopen never triggers automatic theme application.”

In `README.md`, state all four facts together:

1. the official ChatGPT/Codex entry is unchanged;
2. the user opens Codex Assistant and clicks “应用主题”;
3. the selection is remembered but a full ordinary reopen needs another click;
4. there is no alternate shortcut or background process.

- [ ] **Step 4: Add the 0.10.0 changelog entry**

At the top of `CHANGELOG.md`, add:

```markdown
## 0.10.0 — 2026-07-22

- 移除“Codex（主题版）”启动入口、短时启动器和相关环境契约；官方 ChatGPT/Codex 入口保持不变。
- 主题只在用户打开 Codex Assistant 并点击“应用主题”后生效；完全关闭并从官方入口重开后需要再次手动应用。
- 升级时仅在目标确认为当前 Codex Assistant 可执行文件时清理旧版主题快捷方式，不修改官方快捷方式、安装包或本地数据库。
- 保留受验证的 Store 进程识别、显式重启确认、旧进程树完全退出与稳定 app-server 检查。
```

- [ ] **Step 5: Scan documentation for contradictory claims**

```powershell
rg -n "Codex（主题版）|重启后仍保持|自动恢复主题|自动重新应用|持久入口" README.md CONTEXT.md CHANGELOG.md docs/adr website
```

Expected: matches remain only in changelog/history, ADR 0004 historical context, ADR 0005 migration explanation, installer retirement cleanup, and tests asserting absence. Current product guidance contains no automatic-persistence claim.

- [ ] **Step 6: Commit the decision record if isolated**

```powershell
git add docs/adr/0005-on-demand-official-entry-theme-flow.md docs/adr/0004-theme-only-product.md CONTEXT.md README.md CHANGELOG.md
git diff --cached --check
git commit -m "docs: adopt official-entry manual theme flow"
```

Expected: only decision and current product documentation are staged. Skip in a shared dirty worktree.

---

## Task 5: Version, package, and synchronize the website release locally

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/tests/product_identity.rs`
- Modify: `website/package.json`
- Modify: `website/package-lock.json`
- Modify: `website/app/page.tsx`
- Modify: `website/tests/rendered-html.test.mjs`
- Modify: `website/README.md`
- Modify: `website/design-qa.md`
- Create: `website/scripts/sync-installer-proof.mjs`
- Create after build: `website/public/downloads/Codex-Assistant-0.10.0-x64-setup.exe`

- [ ] **Step 1: Bump all package identities to 0.10.0**

Set `0.10.0` in the root `package.json`, root package and empty-root entry in `package-lock.json`, `src-tauri/Cargo.toml`, the root `codex-assistant` entry in `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Update all exact version assertions in `src-tauri/tests/product_identity.rs`.

Use package managers only for lockfile normalization:

```powershell
npm install --package-lock-only --ignore-scripts
npm --prefix website install --package-lock-only --ignore-scripts
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: lockfiles remain parseable, the Rust package resolves as 0.10.0, and no dependency is upgraded merely to perform the version bump.

- [ ] **Step 2: Update website behavior claims before changing artifact proof**

In `website/app/page.tsx`:

- Replace all 0.9.0 labels with 0.10.0.
- Rename principle 03 to `选择保留，应用由你决定`.
- Use this paragraph: `主题选择会保留；完全关闭并从官方入口重新打开 ChatGPT/Codex 后，再到 Codex Assistant 点击一次“应用主题”。没有后台驻留或自动重启。`
- Replace the desktop-boundary paragraph with: `桌面版检测官方 Microsoft Store ChatGPT/Codex、窗口数量和主题会话。它不改变官方入口；每次完整重开后，由你回到 Codex Assistant 明确点击“应用主题”。`
- Replace the feature item `用户主动启动的持久主题入口` with `官方入口不变，主题按需手动应用`.
- Point the download to `/downloads/Codex-Assistant-0.10.0-x64-setup.exe`.

In `website/tests/rendered-html.test.mjs`, require the new manual-apply copy and add:

```js
assert.doesNotMatch(html, /Codex（主题版）/);
assert.match(html, /官方入口不变/);
assert.match(html, /再次点击“应用主题”/);
assert.match(html, /Codex-Assistant-0\.10\.0-x64-setup\.exe/);
```

- [ ] **Step 3: Add deterministic installer synchronization**

Create `website/scripts/sync-installer-proof.mjs` with this complete implementation:

```js
import { createHash } from "node:crypto";
import { copyFile, readFile, stat, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const websiteRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const repositoryRoot = resolve(websiteRoot, "..");
const source = resolve(
  repositoryRoot,
  "src-tauri",
  "target",
  "release",
  "bundle",
  "nsis",
  "Codex Assistant_0.10.0_x64-setup.exe",
);
const destination = resolve(
  websiteRoot,
  "public",
  "downloads",
  "Codex-Assistant-0.10.0-x64-setup.exe",
);
const pagePath = resolve(websiteRoot, "app", "page.tsx");
const testPath = resolve(websiteRoot, "tests", "rendered-html.test.mjs");

await copyFile(source, destination);
const bytes = await readFile(destination);
const size = (await stat(destination)).size;
const sha256 = createHash("sha256").update(bytes).digest("hex");
const mib = `${(size / 1_048_576).toFixed(2)} MiB`;
const groupedBytes = new Intl.NumberFormat("en-US").format(size);

let page = await readFile(pagePath, "utf8");
page = page.replace(
  /<span data-installer-size>[^<]+<\/span>/,
  `<span data-installer-size>${mib}</span>`,
);
page = page.replace(
  /<p data-installer-proof>[^<]+<\/p>/,
  `<p data-installer-proof>0.10.0 · ${groupedBytes} bytes · SHA-256 · ${sha256.slice(0, 12)}…${sha256.slice(-8)}</p>`,
);
await writeFile(pagePath, page, "utf8");

let testSource = await readFile(testPath, "utf8");
testSource = testSource.replace(/const installerBytes = \d+;/, `const installerBytes = ${size};`);
testSource = testSource.replace(
  /const installerSha256 = "[a-f0-9]{64}";/,
  `const installerSha256 = "${sha256}";`,
);
await writeFile(testPath, testSource, "utf8");

process.stdout.write(`${basename(destination)}\n${size}\n${sha256}\n`);
```

At the top of `website/tests/rendered-html.test.mjs`, add concrete generated constants that the script owns:

```js
const installerBytes = 0;
const installerSha256 = "0000000000000000000000000000000000000000000000000000000000000000";
```

Replace hard-coded old size/hash assertions with `installerBytes` and `installerSha256`. The zero values are an intentional red-test seed, not release metadata; the synchronization script must replace them from the built binary before the website test may pass.

- [ ] **Step 4: Verify the website is red until the 0.10.0 binary exists**

```powershell
npm --prefix website test
```

Expected: the test fails because the 0.10.0 installer is not yet synchronized and the generated constants are still the red-test seed.

- [ ] **Step 5: Run the complete repository quality gate before packaging**

```powershell
npm run check
```

Expected: TypeScript, lint, formatting, Rust Clippy, Vitest, and Rust tests all pass. If formatting checks fail, run `npm run fmt`, inspect the diff, then rerun `npm run check`.

- [ ] **Step 6: Build the unsigned current-user NSIS package**

```powershell
npm run tauri build -- --bundles nsis
```

Expected: exit 0 and the exact file `src-tauri/target/release/bundle/nsis/Codex Assistant_0.10.0_x64-setup.exe` exists. Do not launch or install it during this step.

- [ ] **Step 7: Synchronize exact artifact proof and verify the website**

```powershell
node website/scripts/sync-installer-proof.mjs
npm --prefix website test
npm --prefix website run build
Get-FileHash "website/public/downloads/Codex-Assistant-0.10.0-x64-setup.exe" -Algorithm SHA256
```

Expected: the script prints the filename, a non-zero byte count, and a 64-character SHA-256; the website test/build pass; the PowerShell hash matches `installerSha256` in the rendered HTML test.

- [ ] **Step 8: Scan the shipped code and website for the retired behavior**

```powershell
rg -n -- "--launch-themed-codex|CreateShortCut.*Codex（主题版）|install-launcher|launch-themed-codex|restart-themed-codex|launcher_installed|themed-launcher" src src-tauri shared website
```

Expected: no current implementation or current marketing matches. Only target-verified legacy cleanup and absence assertions may mention the retired shortcut filename.

- [ ] **Step 9: Commit the release slice if isolated**

```powershell
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json src-tauri/tests/product_identity.rs website/package.json website/package-lock.json website/app/page.tsx website/tests/rendered-html.test.mjs website/README.md website/design-qa.md website/scripts/sync-installer-proof.mjs website/public/downloads/Codex-Assistant-0.10.0-x64-setup.exe
git diff --cached --check
git commit -m "release: prepare Codex Assistant 0.10.0"
```

Expected: only 0.10.0 release metadata, current website copy, generated proof, and the new installer are staged. Skip in a shared dirty worktree.

---

## Task 6: Perform non-destructive Windows acceptance with a user checkpoint

**Files:**

- Modify only if defects are found: files from Tasks 1–5
- Append after verified completion: `D:\Work_plan\README.md`

- [ ] **Step 1: Capture the official entry and background-state baseline without changing anything**

Run read-only checks:

```powershell
$shell = New-Object -ComObject WScript.Shell
$programs = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
Get-ChildItem -LiteralPath $programs -Filter '*.lnk' -File | ForEach-Object {
  $shortcut = $shell.CreateShortcut($_.FullName)
  [pscustomobject]@{ Name = $_.Name; Target = $shortcut.TargetPath; Arguments = $shortcut.Arguments }
} | Sort-Object Name
Get-CimInstance Win32_StartupCommand | Where-Object { $_.Name -match 'Codex Assistant|Dream Skin' }
Get-ScheduledTask | Where-Object { $_.TaskName -match 'Codex Assistant|Dream Skin' }
```

Expected: capture the exact official entry for later comparison; no Codex Assistant/Dream Skin startup command or scheduled task is introduced by 0.10.0.

- [ ] **Step 2: Stop and request explicit authorization before installation/runtime QA**

Explain that installing 0.10.0 changes the current-user Codex Assistant installation and removes only a prior target-verified `Codex（主题版）` shortcut. Also explain that a real guarded restart test must not terminate the Codex window carrying the current task. Continue only after the user selects a safe test window/session and authorizes installation.

- [ ] **Step 3: Install 0.10.0 and verify entry ownership**

After authorization, install `website/public/downloads/Codex-Assistant-0.10.0-x64-setup.exe`, then rerun the shortcut inventory from Step 1.

Expected:

- the official ChatGPT/Codex entry target and arguments are unchanged;
- there is no `Codex（主题版）.lnk` targeting Codex Assistant;
- Codex Assistant has no startup, tray, watcher, supervisor, or scheduled-task registration;
- closing Codex does not start it again.

- [ ] **Step 4: Verify the manual-reapply lifecycle in a safe secondary session**

Perform this exact sequence without using the current task-bearing Codex process:

1. Open official ChatGPT/Codex normally and confirm official appearance.
2. Open Codex Assistant, select `Aurora Grid`, and click `应用主题`.
3. If a restart prompt appears, confirm only after verifying the secondary session has no unfinished work.
4. Verify the task list, settings, sidebar, composer, buttons, icons, keyboard focus, scrolling, links, and account page remain usable.
5. Close official ChatGPT/Codex completely; verify it stays closed for at least 15 seconds.
6. Reopen it from the unchanged official entry; verify Codex Assistant does not auto-start and the visual theme is not claimed as applied.
7. Open Codex Assistant; verify `Aurora Grid` is still selected and the UI instructs the user to click `应用主题`.
8. Click `应用主题` and verify the theme becomes visible again.
9. Restore official appearance and verify only Codex Assistant-owned live styles are removed.

Expected: selection persists, application is explicit per official-app session, and no normal Codex capability or data access is lost.

- [ ] **Step 5: Re-run the complete automated gates after any runtime fix**

```powershell
npm run check
npm run tauri build -- --bundles nsis
node website/scripts/sync-installer-proof.mjs
npm --prefix website test
npm --prefix website run build
```

Expected: all commands pass and the final website installer proof matches the rebuilt artifact.

- [ ] **Step 6: Review the final diff against the accepted scope**

```powershell
git status --short
git diff --check
rg -n "Codex（主题版）|launch-themed-codex|launcher_installed|themed-launcher" src src-tauri shared website README.md CONTEXT.md docs/adr CHANGELOG.md
```

Expected: no whitespace errors; current product code and current marketing expose only the official-entry/manual-apply flow; historical and target-verified retirement references are clearly labeled.

- [ ] **Step 7: Update the work record only after the whole task is genuinely complete**

Check the 2026-07-22 section in `D:\Work_plan\README.md` first. Append or locally update a single non-duplicate entry with the actual 0.10.0 changes, exact verification commands, final installer path/hash, runtime QA result, risks, and any separately pending website deployment. Do not record credentials, cookies, full private database paths, or unverified outcomes.

- [ ] **Step 8: Report completion and the remaining deployment boundary**

Report:

- final automated test results;
- runtime QA result and which safe session was used;
- exact installer path, byte count, and SHA-256;
- confirmation that the official entry was unchanged and no alternate/background path exists;
- confirmation that selection persists but each ordinary full reopen requires another explicit apply;
- whether the public website was only built locally or separately deployed under explicit authorization.

## Final Self-Review Checklist

- [ ] Every accepted requirement is represented by a test, code change, documentation change, or explicit runtime acceptance step.
- [ ] `ThemeEnvironmentReport` has one Rust shape and one TypeScript shape, both contract version 2, with identical serialized names and no launcher field.
- [ ] No placeholder markers, disabled tests, skipped assertions, or silently broadened permissions remain.
- [ ] The source contains no alternate-entry execution route and the installer creates no alternate Codex shortcut.
- [ ] Upgrade cleanup is target-verified and cannot delete an unrelated or official shortcut by filename alone.
- [ ] Theme application remains a direct result of the user's current action; polling and startup perform read-only inspection only.
- [ ] Recent residual-process, stable-appserver, SQLite-safety, DOM-compatibility, and theme rollback tests remain green.
- [ ] Website claims and installer proof describe the same 0.10.0 binary that was built.
- [ ] Real installation, official-app restart, and public deployment occur only after separate explicit authorization.
