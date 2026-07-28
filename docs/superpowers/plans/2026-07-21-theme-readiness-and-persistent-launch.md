# Theme Readiness and Persistent Launch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Codex Assistant detect whether the local Microsoft Store Codex can be themed, explain and repair unsupported states, apply a selected theme without developer setup, and reapply it whenever the user intentionally launches Codex through the installed themed shortcut.

**Architecture:** Add a pure readiness classifier around the verified Store package, running UI process, owned CDP session and installed themed shortcut. Replace direct execution of the protected `WindowsApps` binary with Windows' official `IApplicationActivationManager::ActivateApplication` API, then expose a short-lived `--launch-themed-codex` mode that launches, verifies, applies the saved preference and exits. The installer creates a normal Start-menu shortcut for that mode; no tray process, login task, supervisor, automatic relaunch or official package modification is introduced.

**Tech Stack:** React 19, TypeScript 7, Tauri 2, Rust 1.82, Windows COM/Store activation, NSIS, Vitest, Cargo integration tests, Playwright/CDP.

## Global Constraints

- Never modify `WindowsApps`, `app.asar`, Codex settings, signed package files or the user's official Codex shortcut.
- Codex starts only after an explicit user click in Codex Assistant or on the installed `Codex（主题版）` shortcut.
- Closing Codex never causes an automatic restart. No tray icon, login task, scheduled supervisor or background watcher is installed.
- A normal Codex instance without the verified loopback CDP endpoint is reported as `restart-required`; it is never presented as theme-ready.
- The selected theme persists in the existing current-user state directory and is reapplied after each intentional themed launch.
- Store activation uses the exact official AppUserModelID and a random loopback-only debugging port; package, owner, process and listener verification remain mandatory.
- Active Codex work blocks automatic restart. Force termination remains an explicit, expiring confirmation flow.
- Other computers receive all required executable code and bundled assets in the installer; no developer AppData path, source checkout or local script is required.
- The existing dirty worktree is preserved; do not reset, overwrite unrelated changes or commit without a clean task boundary.

---

### Task 1: Red-capable environment readiness contract

**Files:**

- Create: `src-tauri/src/theme_environment.rs`
- Modify: `src-tauri/src/theme_app.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `shared/theme-types.ts`
- Modify: `src/lib/themeApi.ts`
- Test: `src-tauri/tests/theme_environment.rs`
- Test: `src/lib/themeApi.test.ts`

**Interfaces:**

- Consumes: `ThemeEnvironmentProbe { platform_supported, package_version, verified_process_count, session_reachable, launcher_installed, selected_theme_id }`.
- Produces: `ThemeEnvironmentReport { contract_version, status, checks, codex_version, verified_process_count, session_reachable, launcher_installed, selected_theme_id, next_action }`.
- `ThemeEnvironmentStatus`: `ready | codex-not-running | restart-required | setup-required | unsupported`.
- `ThemeNextAction`: `apply-now | launch-themed-codex | restart-themed-codex | install-codex | close-extra-windows | none`.

- [ ] **Step 1: Write a failing pure Rust classifier test for the reproduced stale-session state.**

```rust
let report = classify_environment(ThemeEnvironmentProbe {
    platform_supported: true,
    package_version: Some("26.715.8383.0".into()),
    verified_process_count: 1,
    session_reachable: false,
    launcher_installed: true,
    selected_theme_id: Some("aurora-grid".into()),
});
assert_eq!(report.status, ThemeEnvironmentStatus::RestartRequired);
assert_eq!(report.next_action, ThemeNextAction::RestartThemedCodex);
```

- [ ] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml --test theme_environment` and verify it fails because the contract does not exist.**
- [ ] **Step 3: Implement the classifier and a Windows runtime probe; represent every check with stable codes rather than backend-localized prose.**
- [ ] **Step 4: Add `get_theme_environment` to the Tauri handler and ACL, parse contract version 1 strictly in TypeScript, and reject unknown fields/codes.**
- [ ] **Step 5: Run the focused Rust and TypeScript API tests and verify the stale session is reported as `restart-required`, not generic failure.**

### Task 2: Official Store activation and first-session launch

**Files:**

- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/control_layer/windows_package.rs`
- Modify: `src-tauri/src/theme_app.rs`
- Test: `src-tauri/tests/windows_package_identity.rs`
- Test: `src-tauri/tests/theme_application.rs`

**Interfaces:**

- Produces: `activate_store_codex(app_user_model_id: &str, arguments: &[String]) -> Result<u32, IdentityError>`.
- Produces: `launch_verified_codex(package, reservation, timeout_ms) -> Result<RestartedSession, IdentityError>` for the zero-running-window case.
- Existing `restart_verified_codex` closes a verified root only after safety authorization, then delegates replacement launch to the same Store activation primitive.

- [ ] **Step 1: Add a failing unit test proving launch arguments are one bounded Windows argument string and contain only the two loopback CDP switches.**
- [ ] **Step 2: Add a failing application test proving zero running Codex windows chooses launch, one unmanaged window chooses restart, and more than one window fails closed.**
- [ ] **Step 3: Run the tests and verify the current direct `Command::new(package.executable)` seam fails the new Store-activation contract.**
- [ ] **Step 4: Implement `IApplicationActivationManager::ActivateApplication` using COM initialization, the exact `OpenAI.Codex_2p2nqsd0c76g0!App` AUMID, `AO_NONE`, checked HRESULTs and deterministic interface release.**
- [ ] **Step 5: Replace direct WindowsApps execution, wait for the exact returned verified process, then verify listener PID, loopback address and browser identity before saving a session.**
- [ ] **Step 6: Map access/activation failures to distinct readiness reasons so the UI can give a concrete remedy.**
- [ ] **Step 7: Run focused package, CDP and theme application tests; run the ignored real-package discovery test without modifying the package.**

### Task 3: Short-lived persistent themed launcher

**Files:**

- Create: `src-tauri/src/theme_launcher.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/theme_app.rs`
- Modify: `src-tauri/windows/installer-hooks.nsh`
- Test: `src-tauri/tests/theme_launcher.rs`
- Test: `src-tauri/tests/product_identity.rs`

**Interfaces:**

- Produces: `run_theme_launcher() -> ThemeLauncherExit` for `codex-assistant.exe --launch-themed-codex`.
- `ThemeLauncherExit`: `Applied | LaunchedOfficialWithoutPreference | CodexAlreadyRunningUnmanaged | Unsupported | Failed`.
- The launcher acquires an exclusive current-user operation lock, loads the saved preference, launches Codex only when no verified UI process exists, applies until the main DOM is ready, and exits.

- [ ] **Step 1: Add failing tests for selected-theme launch/apply, no-preference guidance, unmanaged-running refusal, duplicate launcher exclusion and no relaunch after Codex closes.**
- [ ] **Step 2: Implement the CLI branch before Tauri initialization so successful launches create no Assistant window or tray/taskbar process.**
- [ ] **Step 3: Reuse `ThemeApplication` launch/apply primitives and persist the fresh owned session; never loop after the bounded apply deadline.**
- [ ] **Step 4: Extend the NSIS post-install hook to create `$SMPROGRAMS\Codex（主题版）.lnk` targeting the installed Assistant binary with `--launch-themed-codex`; remove only that exact shortcut during uninstall.**
- [ ] **Step 5: Add package tests asserting the CLI branch and installer shortcut ship, and that no Startup-folder link, Run key or scheduled task is created.**
- [ ] **Step 6: Run focused launcher and installer contract tests.**

### Task 4: Environment panel, specific guidance and one-click recovery

**Files:**

- Modify: `shared/theme-types.ts`
- Modify: `src/lib/themeApi.ts`
- Modify: `src/hooks/useTheme.ts`
- Modify: `src/components/ThemesPage.tsx`
- Modify: `src/styles/global.css`
- Test: `src/lib/themeApi.test.ts`
- Test: `src/hooks/useTheme.test.ts`
- Test: `src/components/ThemesPage.test.tsx`

**Interfaces:**

- `useTheme()` exposes `environment`, `refreshEnvironment()` and the existing mutation methods.
- UI maps stable check codes to Chinese titles, explanations and exact next steps.

- [ ] **Step 1: Add failing component tests for all five overall states and exact actions: install Codex, start themed Codex, explicitly restart, close extra windows and apply now.**
- [ ] **Step 2: Add a failing hook test proving an operation refreshes both snapshot and environment, and a stale session never collapses into the generic “主题操作失败” banner.**
- [ ] **Step 3: Implement an always-visible “本机环境检测” panel with pass/action/fail rows, detected version, saved theme and themed-shortcut status.**
- [ ] **Step 4: Keep theme cards selectable while Codex is closed; save the preference first, then let the same click launch and apply through the verified path.**
- [ ] **Step 5: For an unmanaged running Codex, explain that live attachment is impossible and offer the existing guarded explicit restart; never hide the reason behind generic copy.**
- [ ] **Step 6: Add “以后从开始菜单打开 Codex（主题版）” persistence guidance after success without claiming that the official shortcut was modified.**
- [ ] **Step 7: Run focused Vitest tests and keyboard/accessibility checks.**

### Task 5: End-to-end persistence, release and distribution

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `CONTEXT.md`
- Modify: `docs/adr/0004-theme-only-product.md`
- Modify: `website/app/page.tsx`
- Modify: `website/tests/rendered-html.test.mjs`
- Create: `website/public/downloads/Codex-Assistant-0.9.0-x64-setup.exe`
- Create: `outputs/codex-assistant-0.9.0-environment-ready.png`

**Interfaces:**

- Produces: version `0.9.0` NSIS installer and matching public download.

- [ ] **Step 1: Run the Phase-1 command `powershell.exe -File docs/superpowers/diagnostics/2026-07-21-theme-readiness-repro.ps1`; keep it red before installation.**
- [ ] **Step 2: Run full `npm run check`, build NSIS and verify product/file versions, installer contents, shortcut arguments and absence of autorun artifacts.**
- [ ] **Step 3: Install 0.9.0 for the current user without restarting the current Codex task; inspect the environment panel and screenshot it.**
- [ ] **Step 4: At a user-approved restart checkpoint, close only a disposable/secondary Codex instance or wait for the user to close the main instance; launch from `Codex（主题版）`, verify CDP identity, selected theme visibility, normal text/icons/composer interactions and helper exit.**
- [ ] **Step 5: Close Codex manually and prove no process relaunches it; click the themed shortcut again and verify the same theme reapplies without opening Codex Assistant.**
- [ ] **Step 6: Re-run the Phase-1 command and require `ready_for_one_click_theme=true` while the themed session is alive; add a cold-state assertion that reports `codex-not-running` rather than failure after manual close.**
- [ ] **Step 7: Update the public site and installer metadata, deploy the exact build, then re-download and verify byte length and SHA-256.**
- [ ] **Step 8: Remove diagnostic-only instrumentation and temporary profiles, update `D:\Work_plan\README.md`, and report the unsigned-installer SmartScreen risk.**

## Self-Review

- Spec coverage: local detection, actionable guidance, one-click repair, clean-machine Store activation, persistent selected theme, user-controlled launch, no taskbar/background supervisor, restart persistence, full distribution and public installer all map to tasks.
- Placeholder scan: every implementation and validation step is concrete; no deferred placeholders remain.
- Type consistency: environment enums, report fields, Store activation functions and launcher exit states are defined before their consumers.
- Safety boundary: the only persistent object is a normal Start-menu shortcut; no official package file, official shortcut, login startup location or background process is modified.
- Honesty boundary: the product will state that persistence applies when Codex is opened through `Codex（主题版）`; it will detect and explain an ordinary unmanaged launch instead of falsely reporting success.
