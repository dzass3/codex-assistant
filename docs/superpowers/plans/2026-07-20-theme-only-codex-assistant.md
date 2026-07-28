# Theme-Only Codex Assistant Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Codex Assistant into a Windows theme-only utility whose bundled themes work on clean installations, whose local-image import is safe and device-local, and whose decoration never obscures Codex text, icons, controls, menus, or input.

**Architecture:** Remove Live Agents and Smart Routing from every product surface and retire their installed Codex configuration on upgrade. Keep only the internal official-process verification, loopback CDP session, active-work restart guard, theme catalog, and theme injection primitives; expose them through a theme-specific application/API. Generate only audited CSS owned by the app, import only validated local image bytes, and make multi-page application fail closed to a consistent official appearance.

**Tech Stack:** React 19, TypeScript 7, Tauri 2, Rust 1.82, Vitest/Testing Library, Cargo integration tests, Playwright over the real Codex CDP session, NSIS, OpenAI Sites.

## Global Constraints

- Product UI and public website expose only one-click Codex themes; no Live Agents, Smart Routing, model-routing, preflight, or injected routing controls remain.
- Keep official Codex package identity verification and random loopback-only CDP; never patch `app.asar`, WindowsApps, or signed official files.
- Bundled assets require verified commercial redistribution rights. Arina and other user-provided images remain local-only and are excluded from Git, installers, website assets, and releases.
- Imported content is a single JPEG, PNG, or WebP image. No remote URL, user CSS, HTML, JavaScript, or arbitrary manifest is accepted.
- Theme CSS must not set semantic foreground tokens, generic `color`/`fill` on controls, or stacking/interaction properties on Codex-owned nodes.
- A failed multi-page application leaves every discovered compatible page on the official appearance and never reports the theme as applied.
- Preserve existing `%APPDATA%/codex-agent-monitor/routing/` theme preferences, local themes, and verified theme session during the 0.8.0 migration.
- The dirty worktree is preserved; no reset, checkout, or unrelated rewrite is permitted.

---

### Task 1: Theme-only desktop surface

**Files:**

- Modify: `src/App.tsx`
- Modify: `src/config.ts`
- Modify: `src/styles/global.css`
- Modify: `src/components/ThemesPage.tsx`
- Modify: `src/components/ThemesPage.test.tsx`
- Delete: `src/components/AppNavigation.tsx`
- Delete: `src/components/AppNavigation.test.tsx`
- Delete: `src/components/SmartRoutingPage.tsx`
- Delete: `src/components/SmartRoutingPage.test.tsx`
- Delete: `src/hooks/useRouting.ts`
- Delete: `src/hooks/useRouting.test.ts`
- Delete: `src/control/routingControlHarness.ts`
- Delete: `src/control/routingControlHarness.test.ts`

**Interfaces:**

- Consumes: `useTheme(): ThemeController`.
- Produces: a single `ThemesPage` startup surface with refresh, import, apply, and restore controls.

- [ ] **Step 1: Write a failing app-level test**

```tsx
render(<App />);
expect(screen.getByRole("heading", { name: "一键换肤" })).toBeInTheDocument();
expect(screen.getByRole("button", { name: "导入本机图片" })).toBeInTheDocument();
expect(screen.queryByText("Smart Routing")).not.toBeInTheDocument();
expect(screen.queryByText("实时代理")).not.toBeInTheDocument();
```

- [ ] **Step 2: Run `npx vitest run src/App.test.tsx src/components/ThemesPage.test.tsx` and verify the new assertions fail.**
- [ ] **Step 3: Replace the tabbed monitor shell with a focused header and `ThemesPage`; remove routing/monitor navigation and CSS selectors.**
- [ ] **Step 4: Run the same Vitest command and verify it passes.**
- [ ] **Step 5: Stage only Task 1 files for review; do not commit over unrelated dirty changes.**

### Task 2: Safe local-image import

**Files:**

- Modify: `shared/theme-types.ts`
- Modify: `src/lib/themeApi.ts`
- Modify: `src/lib/themeApi.test.ts`
- Modify: `src/hooks/useTheme.ts`
- Modify: `src/hooks/useTheme.test.ts`
- Modify: `src/components/ThemesPage.tsx`
- Modify: `src/components/ThemesPage.test.tsx`
- Modify: `src-tauri/src/local_theme.rs`
- Modify: `src-tauri/tests/local_theme_catalog.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/permissions/default.toml`

**Interfaces:**

- Consumes: `ThemeImportRequest { name: string; image_data_url: string }`.
- Produces: `import_local_theme(request) -> Result<ThemeImportReceipt, String>` where `ThemeImportReceipt { theme_id: string }`.

- [ ] **Step 1: Add failing Rust tests for a valid image, MIME/magic mismatch, oversized bytes, duplicate content, unsafe names, symlink/reparse-point paths, and atomic cleanup after write failure.**

```rust
let receipt = catalog.import_image("My Garden", "image/webp", valid_webp())?;
assert!(receipt.theme_id.starts_with("local-"));
assert_eq!(catalog.packs().len(), 1);
assert_eq!(catalog.asset_bytes(&receipt.theme_id).as_deref(), Some(valid_webp()));
```

- [ ] **Step 2: Run `cargo test --manifest-path src-tauri/Cargo.toml --test local_theme_catalog` and verify the new tests fail.**
- [ ] **Step 3: Implement strict magic-byte validation, SHA-256 ID generation, bounded display names, atomic directory staging, local-only rights metadata, and exact manifest generation.**
- [ ] **Step 4: Add a failing UI test that chooses one local image, imports it, refreshes the catalog, and exposes its card without leaking the original path.**
- [ ] **Step 5: Implement an accessible hidden file input; resize/encode in the WebView to a bounded WebP data URL before IPC; never accept URLs or manifests.**
- [ ] **Step 6: Run the focused Rust and Vitest tests and verify they pass.**
- [ ] **Step 7: Stage only Task 2 files for review; do not commit over unrelated dirty changes.**

### Task 3: Non-obstructive and transactional theme engine

**Files:**

- Modify: `src-tauri/src/theme.rs`
- Modify: `src-tauri/tests/theme_contract.rs`
- Modify: `src-tauri/tests/theme_application.rs`
- Modify: `src-tauri/resources/control/fixtures/local-root.html`
- Modify: `src-tauri/resources/control/fixtures/local-child.html`

**Interfaces:**

- Consumes: a validated declarative `ThemePack` and verified Codex `BrowserEndpoint`.
- Produces: `ThemeApplyResult { applied_pages, scripts }` only after all compatible pages pass; otherwise every page is restored.

- [ ] **Step 1: Add failing CSS-contract tests.**

```rust
for forbidden in [
    "--color-token-foreground:",
    "--color-token-text-primary:",
    "fill:currentColor!important",
    "button[class~=\\\"bg-token-foreground\\\"]",
    "body::before",
] {
    assert!(!source.contains(forbidden), "unsafe theme rule: {forbidden}");
}
```

- [ ] **Step 2: Add failing CDP transaction tests: delayed DOM becomes ready, second compatible page fails and both pages restore, utility pages are ignored, and a successful retry commits every startup script exactly once.**
- [ ] **Step 3: Run `cargo test --manifest-path src-tauri/Cargo.toml --test theme_contract --test theme_application` and verify the new tests fail for the intended reasons.**
- [ ] **Step 4: Generate background on `html` without pseudo-element overlays; theme only surfaces, borders, shadows, and selected-state tint using native surface tokens as the contrast base. Preserve all native text/icon/button colors and all pointer/stacking/overflow behavior.**
- [ ] **Step 5: Replace exact business-button color verification with checks for owned stylesheet connectivity, non-empty background, visible main/sidebar/composer rectangles, and unobstructed composer hit testing.**
- [ ] **Step 6: Implement bounded stability retries and all-target rollback to official appearance before returning `PartialApplication`.**
- [ ] **Step 7: Run the focused Rust tests and verify they pass.**
- [ ] **Step 8: Stage only Task 3 files for review; do not commit over unrelated dirty changes.**

### Task 4: Retire Smart Routing while preserving theme safety

**Files:**

- Create: `src-tauri/src/private_state.rs`
- Create: `src-tauri/src/theme_app.rs`
- Create: `src-tauri/src/legacy_routing_cleanup.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/theme.rs`
- Modify: `src-tauri/src/local_theme.rs`
- Modify: `src-tauri/src/control_layer/cdp.rs`
- Modify: `src-tauri/src/control_layer/mod.rs`
- Modify: `src-tauri/permissions/default.toml`
- Modify: `src-tauri/tauri.conf.json`
- Delete: `src-tauri/src/routing_app.rs`
- Delete: `src-tauri/src/routing/`
- Delete: `src-tauri/src/routing_mcp/`
- Delete: `src-tauri/src/preflight/`
- Delete: `src-tauri/src/control_layer/injector.rs`
- Delete: `src-tauri/resources/routing/`
- Delete: `src-tauri/resources/control/routing-control.js`
- Delete: `src-tauri/resources/control/routing-control.css`
- Delete: routing/preflight/control-injection integration tests that no longer represent product behavior.

**Interfaces:**

- Consumes: official-process identity and `MonitorRuntime` only for internal active-work restart protection.
- Produces: `ThemeApplication` commands: snapshot, preview, import, start session, activate, restore, prepare/cancel force restart.

- [ ] **Step 1: Add failing ACL/product-identity tests asserting that no routing command, routing resource, routing-mcp subcommand, model profile, skill, or UI string ships.**
- [ ] **Step 2: Add a migration test that starts with an owned legacy routing manifest and proves one startup cleanup restores only Codex Assistant-owned entries while retaining themes and unrelated user configuration.**
- [ ] **Step 3: Extract `protect_owned_path` and `replace_existing` into `private_state.rs`; update theme, local catalog, CDP session, and legacy cleanup callers.**
- [ ] **Step 4: Extract the theme session/restart/application fields and methods into `ThemeApplication`, with theme-specific reason/status types and no routing snapshot or route policy.**
- [ ] **Step 5: Run the legacy cleanup once, record a completion marker, remove the routing command/resources/modules, and reduce the Tauri allowlist to the theme-only command surface.**
- [ ] **Step 6: Run `npm run check` and fix only failures caused by this feature retirement.**
- [ ] **Step 7: Stage only Task 4 files for review; do not commit over unrelated dirty changes.**

### Task 5: Real Codex functional and visual QA

**Files:**

- Create: `docs/superpowers/diagnostics/2026-07-20-theme-only-runtime-qa.md`
- Create: `outputs/theme-only-app-0.8.0.png`
- Create: `outputs/theme-codex-dense-state-0.8.0.png`

**Interfaces:**

- Consumes: debug/release desktop app and the current verified official Codex installation.
- Produces: reviewed evidence for the complete apply/restore/import flow and non-obstruction claims.

- [ ] **Step 1: Record the QA inventory: startup view, import, all bundled cards, apply, restore, session restart, force confirmation, dense conversation, menu/dropdown, composer, sidebar, top bar, text, SVG icons, stop/send/permission controls, minimum window, and two off-happy-path states.**
- [ ] **Step 2: Use Playwright with normal clicks and keyboard input to apply every bundled theme and one local import; verify the visible active state and restore cycle.**
- [ ] **Step 3: For each theme, assert visible text/icon/control rectangles, composer hit target, no overlay above interactive centers, and no horizontal overflow; capture representative dense and minimum-window screenshots.**
- [ ] **Step 4: Inspect screenshots separately for clipping, obscured text/icons, weak contrast, broken menus, background dominance, side-panel hierarchy, composer material, and unintended interaction changes.**
- [ ] **Step 5: Complete a 30–90 second exploratory pass through typing, menus, task switching, apply during loading, duplicate import, and restore after a failed apply.**
- [ ] **Step 6: Document exact pass/fail evidence and close only the Playwright-owned diagnostic browser/session.**

### Task 6: Build, clean-install evidence, and public site release

**Files:**

- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `CONTEXT.md`
- Modify: `docs/adr/0002-native-routing-and-cdp-control-layer.md`
- Modify: `website/app/page.tsx`
- Modify: `website/app/layout.tsx`
- Modify: `website/app/ProductDemo.tsx`
- Modify: `website/app/globals.css`
- Modify: `website/tests/rendered-html.test.mjs`
- Modify: `website/package.json`
- Modify: `website/package-lock.json`
- Modify: `website/public/og.png`
- Create: `website/public/downloads/Codex-Assistant-0.8.0-x64-setup.exe`

**Interfaces:**

- Consumes: the exact release-mode source and successfully smoke-tested NSIS artifact.
- Produces: version 0.8.0 installer and a Sites deployment whose download bytes match the tested installer.

- [ ] **Step 1: Update product identity and documentation to theme-only language; mark the routing ADR superseded and preserve the theme/CDP decision in a replacement section.**
- [ ] **Step 2: Set every desktop and website version to `0.8.0`; run `npm run check`.**
- [ ] **Step 3: Build the NSIS installer with `npm run tauri build -- --bundles nsis`; verify exactly one expected setup EXE.**
- [ ] **Step 4: Smoke-test the installer from a clean Windows user or equivalent clean state: 12 bundled cards, apply/restore, local import, no routing files/controls/config entries, and no dependency on the developer AppData directory.**
- [ ] **Step 5: Copy that exact EXE to the website download directory, calculate size and SHA-256, and assert both literals in the website test.**
- [ ] **Step 6: Replace the website's agent/router story and three-tab demo with the focused one-click theme story; generate and inspect one matching social card.**
- [ ] **Step 7: Run the website build/tests and perform the requested browser QA against the desktop and responsive public pages.**
- [ ] **Step 8: Save and deploy the exact validated website source through the existing Sites project; poll to success and verify the public installer response has the expected size and SHA-256.**
- [ ] **Step 9: Append the verified result to `D:\Work_plan\README.md` once, then stage release files for review without overwriting unrelated history.**

## Self-Review

- Spec coverage: Smart Routing and Live Agents removal, theme-only UI, cross-device bundled themes, safe local import, non-obstructive CSS, transactional apply, clean-install verification, installer, website copy, website binary, deployment, and work log each map to a task.
- Placeholder scan: no TBD/TODO/“similar to” steps remain.
- Type consistency: `ThemeImportRequest`, `ThemeImportReceipt`, `ThemeApplication`, and theme-only operation/restart types are introduced before their consumers.
- Rights boundary: no task packages Arina; it is exercised only as a local user-owned import when the user supplies it on that machine.
