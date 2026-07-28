# Codex Assistant Observer Restoration and Safe Themes — Implementation Tickets

- Status: Ready for implementation
- Date: 2026-07-22
- Source specification: `docs/superpowers/specs/2026-07-22-codex-assistant-observer-and-safe-themes.md`
- Delivery boundary: source, tests, and local Windows installer; real official-app acceptance and public release remain separately authorized gates

## Execution Rules

- Treat the source specification as authoritative. A ticket may refine implementation detail but may not broaden product scope.
- Use test-first slices: add or restore the smallest failing contract test, prove the red state, implement, then rerun focused and full gates.
- Preserve all unrelated changes in the current dirty worktree. Do not restore deleted files wholesale from `HEAD`; copy only reviewed observer behavior and remove every routing dependency.
- Do not commit in the shared dirty worktree unless the user later requests a commit and the staged set is proven task-owned.
- Do not close, restart, inject into, or install against the current official ChatGPT/Codex task window.
- Do not install the local NSIS artifact or update the public website before Ticket T10 is separately authorized.
- Smart Routing, agent control, alternate Codex entry, tray supervision, auto-relaunch, and startup theme application remain prohibited.
- After every code/configuration slice, run proportionate focused tests. Before packaging, run `npm run check` and `npm run qa:theme-mock`.

## Dependency Map

```text
T01 Observer contract
 ├─> T02 Rust IPC and permissions
 └─> T03 Read-only observer UI
       └─> T04 Two-page product shell

T02 + T03 ─> T05 Restart protection integration

T06 Page classification
 └─> T07 Safe visual hierarchy and local imports
       └─> T08 Full Mock compatibility matrix

T04 + T05 + T08 ─> T09 Documentation, identity, and local installer
T09 ─> T10 Separately authorized real acceptance and public release
```

## Ticket Summary

| ID  | Title                                                    | Depends on    | Primary outcome                                      |
| --- | -------------------------------------------------------- | ------------- | ---------------------------------------------------- |
| T01 | Restore the strict observer contract                     | —             | Shared metadata-only types and parser                |
| T02 | Re-expose read-only monitor IPC, events, and permissions | T01           | Backend snapshots reach the frontend safely          |
| T03 | Restore the read-only real-time agent experience         | T01, T02      | Agent hierarchy, filters, health, and settings       |
| T04 | Build the two-page shell and page-memory boundary        | T03           | `实时代理` + `一键换肤`, no routing surface          |
| T05 | Integrate observer health with guarded theme restarts    | T02, T03      | Active/uncertain work prevents unsafe restart        |
| T06 | Add explicit official-page classification                | —             | Main, utility, sensitive, and unknown behavior       |
| T07 | Deepen safe theme visuals and local-import presentation  | T06           | Dream-Skin-inspired depth without semantic overrides |
| T08 | Expand the Mock compatibility and regression matrix      | T07           | All 12 themes plus local/utility/sensitive coverage  |
| T09 | Align product records and build the next local installer | T04, T05, T08 | Fully checked local deliverable                      |
| T10 | Run real acceptance and publish only after authorization | T09           | Verified installer and synchronized public download  |

---

## T01 — Restore the Strict Observer Contract

**Status:** Ready

**Objective:** Restore shared monitor types and a strict frontend parser without restoring routing types, commands, controls, or content-bearing fields.

**Primary files:**

- Restore and revise: `shared/monitor-types.ts`
- Restore and revise: `src/lib/monitorApi.ts`
- Restore and revise: `src/lib/monitorApi.test.ts`
- Inspect only unless a defect is found: `src-tauri/src/monitor/model.rs`
- Modify if required for contract consistency: `src-tauri/tests/monitor_fixture.rs`

**Test-first steps:**

- [ ] Restore parser tests for a valid metadata-only snapshot.
- [ ] Add rejection tests for malformed IDs, unknown statuses, invalid numeric fields, and raw/unexpected object fields.
- [ ] Add a privacy assertion that the public TypeScript contract contains no prompt, response, reasoning text, tool payload, command output, full path, credential, or routing field.
- [ ] Add truthfulness cases for `turn-context`, `state-database`, `requested-only`, and `unknown` sources.
- [ ] Prove the focused test is red while `shared/monitor-types.ts` and `monitorApi.ts` are absent.

**Implementation steps:**

- [ ] Restore only the observer types listed in the specification.
- [ ] Keep effective and requested models separate.
- [ ] Parse unknown or malformed values into safe explicit states rather than trusting the payload.
- [ ] Restore snapshot, settings, refresh, source-selection, and subscription API calls only.
- [ ] Do not import or recreate `routing-types.ts`.

**Verification:**

```powershell
npx vitest run src/lib/monitorApi.test.ts
rg -n "routing|prompt|response|reasoning_text|tool_output|command_output" shared/monitor-types.ts src/lib/monitorApi.ts
```

**Acceptance:**

- [ ] Valid snapshots preserve every allowed field.
- [ ] Invalid payloads fail closed or normalize to explicit safe values.
- [ ] Requested-only model data is never presented as effective.
- [ ] No routing or conversation-content type is restored.

---

## T02 — Re-expose Read-only Monitor IPC, Events, and Permissions

**Status:** Blocked by T01

**Objective:** Connect the existing `MonitorRuntime` to the frontend through a minimal, synchronized Tauri command and event surface.

**Primary files:**

- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/permissions/default.toml`
- Modify: `src-tauri/tests/acl_consistency.rs`
- Create or restore focused contract test: `src-tauri/tests/monitor_ipc_contract.rs`
- Inspect and preserve: `src-tauri/src/monitor/runtime.rs`

**Test-first steps:**

- [ ] Lock the exact allowed commands: `get_monitor_snapshot`, `refresh_monitor`, `get_monitor_settings`, and `set_codex_home`.
- [ ] Lock one namespaced monitor snapshot event and assert no routing command is emitted or permitted.
- [ ] Assert refresh returns only `MonitorSnapshot` and reports whether the stable signature changed.
- [ ] Assert the setup poller may refresh and emit but cannot mutate Codex or apply a theme.
- [ ] Prove the contract test is red against the current theme-only invoke handler and permission file.

**Implementation steps:**

- [ ] Add the four monitor commands to the generated handler.
- [ ] Reuse the already managed `Arc<MonitorRuntime>`.
- [ ] Emit only changed sanitized snapshots from the existing bounded refresh loop.
- [ ] Return sanitized errors from source selection; retain the last valid setting on failure.
- [ ] Add only the four explicit monitor permissions.
- [ ] Keep theme permissions unchanged and keep arbitrary filesystem/shell/CDP permissions absent.

**Verification:**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test monitor_ipc_contract
cargo test --manifest-path src-tauri/Cargo.toml --test acl_consistency
rg -n "routing|spawn_agent|send_message|interrupt_agent" src-tauri/src/lib.rs src-tauri/permissions/default.toml
```

**Acceptance:**

- [ ] Frontend can fetch and subscribe to sanitized snapshots.
- [ ] Commands, generated handlers, permissions, and tests are synchronized.
- [ ] Polling is read-only and emits only on meaningful snapshot change.
- [ ] No routing or agent-control capability is public.

---

## T03 — Restore the Read-only Real-time Agent Experience

**Status:** Blocked by T01 and T02

**Objective:** Restore the useful observer UI while removing every routing branch and making active-first behavior the default.

**Primary files:**

- Restore and revise: `src/hooks/useMonitor.ts`
- Create: `src/hooks/useMonitor.test.ts`
- Restore and revise: `src/components/AgentTree.tsx`
- Restore and revise: `src/components/AgentTree.test.tsx`
- Restore and revise: `src/components/FilterBar.tsx`
- Restore and revise: `src/components/HealthStrip.tsx`
- Restore and revise: `src/components/SettingsDialog.tsx`
- Create: `src/components/MonitorPage.tsx`
- Create: `src/components/MonitorPage.test.tsx`
- Modify: `src/styles/global.css`

**Test-first steps:**

- [ ] Restore a cycle-safe tree fixture with one root and nested children.
- [ ] Assert active-first mode includes starting/running/tracking-error rows and required ancestors.
- [ ] Assert the all-mode toggle reveals idle/interrupted rows.
- [ ] Assert effective model, effort, source, freshness, status, and model drift are accessible.
- [ ] Assert unknown effective model renders `尚未确认`.
- [ ] Assert empty and degraded-source states remain usable.
- [ ] Assert rendered output contains no Smart Routing button, route state, activation status, or agent-control callback.
- [ ] Test subscription cleanup and last-good snapshot retention in `useMonitor`.

**Implementation steps:**

- [ ] Restore `useMonitor` using the strict monitor API.
- [ ] Strip routing imports, props, state, copy, controls, and explanation panels from `AgentTree`.
- [ ] Preserve ancestor-aware filtering and cycle protection.
- [ ] Default filters to active-only.
- [ ] Present source health without raw paths.
- [ ] Keep `CODEX_HOME` override bounded to Codex Assistant settings and display only a safe label.
- [ ] Add responsive styles consistent with the existing theme-management visual language.

**Verification:**

```powershell
npx vitest run src/hooks/useMonitor.test.ts src/components/AgentTree.test.tsx src/components/MonitorPage.test.tsx
rg -n "Smart Routing|routing|onSetRootEnabled|RootRouting" src/components src/hooks/useMonitor.ts
```

**Acceptance:**

- [ ] Agent hierarchy and model provenance are truthful and readable.
- [ ] Active/all filtering behaves exactly as specified.
- [ ] Degradation never leaks raw source details.
- [ ] Observer UI is entirely read-only.

---

## T04 — Build the Two-page Shell and Page-memory Boundary

**Status:** Blocked by T03

**Objective:** Integrate observer and theme management as two independent top-level pages with a bounded local page preference.

**Primary files:**

- Restore and revise: `src/components/AppNavigation.tsx`
- Restore and revise: `src/components/AppNavigation.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/config.ts`
- Modify: `src/styles/global.css`

**Test-first steps:**

- [ ] Assert exactly two tabs: `实时代理` and `一键换肤`.
- [ ] Assert there is no routing tab or retired product copy.
- [ ] Assert first launch defaults to themes.
- [ ] Assert a valid last-page preference is restored.
- [ ] Assert malformed or unknown stored values fall back to themes.
- [ ] Assert switching pages does not call monitor mutation or theme mutation commands.

**Implementation steps:**

- [ ] Define `AppPage` as only `monitor | themes`.
- [ ] Use a namespaced Codex Assistant-owned local key for the last page.
- [ ] Render `MonitorPage` and `ThemesPage` without remount side effects that mutate Codex.
- [ ] Update product tagline to describe safe themes and read-only agent visibility without implying routing.
- [ ] Preserve keyboard tab semantics, focus visibility, reduced motion, and the 720 px minimum desktop boundary.

**Verification:**

```powershell
npx vitest run src/App.test.tsx src/components/AppNavigation.test.tsx
rg -n "Smart Routing|实时代理|一键换肤" src
```

**Acceptance:**

- [ ] Exactly two product pages are reachable.
- [ ] First launch and subsequent page restoration follow the specification.
- [ ] Navigation is accessible and mutation-free.

---

## T05 — Integrate Observer Health with Guarded Theme Restarts

**Status:** Blocked by T02 and T03

**Objective:** Make the visible observer snapshot and the theme restart guard share the same conservative active-work truth.

**Primary files:**

- Modify: `src-tauri/src/lib.rs`
- Modify if required: `src-tauri/src/theme_app.rs`
- Modify: `src/components/ForceRestartDialog.tsx`
- Modify: `src/hooks/useTheme.ts`
- Modify tests: `src/hooks/useTheme.test.ts`
- Modify tests: `src-tauri/tests/theme_application.rs`
- Create focused test if clearer: `src-tauri/tests/theme_restart_observer_guard.rs`

**Test-first steps:**

- [ ] Assert normal restart is blocked when starting or running count is non-zero.
- [ ] Assert degraded/tracking-error source health blocks normal restart when active-work truth is uncertain.
- [ ] Assert a force ticket includes a current bounded impact count.
- [ ] Assert snapshot, process identity, or impact changes invalidate a ticket.
- [ ] Assert no-restart theme switching remains allowed in a verified session.
- [ ] Assert polling never calls start, apply, force-confirm, or restart automatically.

**Implementation steps:**

- [ ] Replace raw count-only helpers with an explicit restart-safety projection containing count and confidence.
- [ ] Return one actionable reason for active work and another for uncertain monitor health.
- [ ] Show the user the current active count and the need to wait or explicitly force.
- [ ] Preserve existing process-tree, owner, Store-package, SQLite, and stable-app-server gates.

**Verification:**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test theme_restart_observer_guard
cargo test --manifest-path src-tauri/Cargo.toml --test theme_application
npx vitest run src/hooks/useTheme.test.ts
```

**Acceptance:**

- [ ] Normal restart cannot interrupt known or uncertain native work.
- [ ] Force restart requires a fresh second confirmation.
- [ ] Theme switching inside a verified session remains one click.

---

## T06 — Add Explicit Official-page Classification

**Status:** Ready

**Objective:** Classify official pages before theme application so rich styling reaches only compatible main/task surfaces.

**Primary files:**

- Modify: `src-tauri/src/theme.rs`
- Modify: `src-tauri/tests/theme_contract.rs`
- Modify: `src-tauri/tests/theme_application.rs`
- Add fixtures under: `src-tauri/resources/control/fixtures/` or test-local HTML constants

**Test-first steps:**

- [ ] Add representative `main-task`, `utility`, `sensitive`, and `unknown` DOM fixtures.
- [ ] Assert main/task pages receive the rich style contract.
- [ ] Assert utility pages receive only the explicitly allowed light treatment or remain official.
- [ ] Assert login, authorization, permission, recovery, and security prompts remain official.
- [ ] Assert unknown DOM returns incompatible and is not modified.
- [ ] Assert utility pages do not block a compatible main task page in the same verified browser session.

**Implementation steps:**

- [ ] Add one internal page-classification result with stable reason codes.
- [ ] Base classification on positive, bounded structural evidence rather than URL substring alone.
- [ ] Require visible/hit-testable main and composer/home-state anchors for rich treatment.
- [ ] Keep classification and verification source generation deterministic and auditable.

**Verification:**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test theme_contract
cargo test --manifest-path src-tauri/Cargo.toml --test theme_application
```

**Acceptance:**

- [ ] Rich styling is impossible on sensitive or unknown pages.
- [ ] Classification failure leaves the page unchanged.
- [ ] Compatible main pages remain independently themeable.

---

## T07 — Deepen Safe Theme Visuals and Local-import Presentation

**Status:** Blocked by T06

**Objective:** Improve background hierarchy, main visual, sidebar, cards, and composer using presentation-only rules inspired by Codex-Dream-Skin.

**Primary files:**

- Modify: `src-tauri/src/theme.rs`
- Modify if required: `shared/theme-catalog.json`
- Modify if required: `src-tauri/src/local_theme.rs`
- Modify tests: `src-tauri/tests/theme_contract.rs`
- Modify tests: `src-tauri/tests/local_theme_catalog.rs`

**Test-first steps:**

- [ ] Lock forbidden selectors and declarations: global semantic tokens, generic `svg`, generic `button`, foreground/fill/stroke overrides, and interactive decorative layers.
- [ ] Lock one owned style/runtime after repeated switching.
- [ ] Lock one coherent composer surface without an opaque inner editor shell.
- [ ] Assert all 12 bundled packs provide a verified visual, safe focal area, layered surfaces, and task-page interference reduction.
- [ ] Add local-import tests for valid raster input, MIME spoofing, oversize, malformed bytes, hash mismatch, and duplicate import.
- [ ] Add a representative imported-theme presentation assertion.

**Implementation steps:**

- [ ] Restrict rules to verified presentation containers and background/border/shadow/backdrop properties.
- [ ] Keep semantic text, content images, icon/SVG ownership, focus, and primary actions untouched.
- [ ] Tune theme-specific backdrop position and overlay strength while keeping shared material behavior consistent.
- [ ] Make sidebar and header quiet enough for navigation labels.
- [ ] Keep task pages less visually dominant than the home surface.
- [ ] Preserve imported bytes and derive presentation metadata without altering the source image.

**Verification:**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --test theme_contract
cargo test --manifest-path src-tauri/Cargo.toml --test local_theme_catalog
```

**Acceptance:**

- [ ] Every bundled theme reaches the common visual-quality bar.
- [ ] No theme changes semantic foreground or intercepts interaction.
- [ ] Local import remains device-only and rejects unsafe input.

---

## T08 — Expand the Mock Compatibility and Regression Matrix

**Status:** Blocked by T07

**Objective:** Extend the existing Playwright harness from the 12-theme main-page contract to local imports and page-classification safety.

**Primary files:**

- Modify: `scripts/qa/mock-theme-contract.mjs`
- Modify: `src-tauri/examples/export_mock_theme_sources.rs`
- Modify if needed: `package.json`
- Generated evidence only: `outputs/mock-theme-qa/`

**Test-first steps:**

- [ ] Retain and run the deliberate full-screen overlay canary; require a covering-layer failure.
- [ ] Add utility, sensitive, and unknown page Mock states.
- [ ] Add one valid local-import theme through the real Rust import/generation path or a hash-verified fixture.
- [ ] Add popover, dialog, menu, dropdown, focus, keyboard, scroll, link, and image interactions.
- [ ] Add contrast/readability bounds for themed surface materials without asserting official semantic recoloring.
- [ ] Prove each new assertion can fail through one bounded canary before trusting the green result.

**Implementation steps:**

- [ ] Apply and verify all 12 bundled themes sequentially in one Mock session.
- [ ] Exercise the representative local import.
- [ ] Assert utility/sensitive/unknown behavior follows classification.
- [ ] Preserve screenshots for baseline, main apply, switch, utility, sensitive, imported, and restore.
- [ ] Emit a machine-readable report with per-theme and per-page results.

**Verification:**

```powershell
$env:MOCK_THEME_CANARY='overlay'; npm run qa:theme-mock
Remove-Item Env:MOCK_THEME_CANARY -ErrorAction SilentlyContinue
1..3 | ForEach-Object { npm run qa:theme-mock; if ($LASTEXITCODE) { exit $LASTEXITCODE } }
```

**Acceptance:**

- [ ] Canary reliably fails for the intended reason.
- [ ] Three consecutive real runs pass every theme and page class.
- [ ] Report proves semantic preservation, hit testing, interaction, switch ownership, and exact restore.

---

## T09 — Align Product Records and Build the Next Local Installer

**Status:** Blocked by T04, T05, and T08

**Objective:** Align current product documentation and identity with the restored observer, then build a fully checked local installer without publishing it.

**Primary files:**

- Modify: `CONTEXT.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Create: `docs/adr/0006-read-only-observer-with-safe-themes.md`
- Modify: `docs/adr/0004-theme-only-product.md`
- Modify: package/Cargo/Tauri version files consistently
- Modify tests: `src-tauri/tests/product_identity.rs`
- Build output: `src-tauri/target/release/bundle/nsis/`

**Test-first steps:**

- [ ] Replace the theme-only product assertion with an exact two-surface assertion.
- [ ] Assert observer commands are present and routing commands/resources remain absent.
- [ ] Assert installer still creates no alternate Codex shortcut, startup entry, tray path, watcher, or scheduled task.
- [ ] Assert website/public copy is not changed by the local build step.

**Implementation steps:**

- [ ] Record ADR 0006 and mark only the product-surface part of ADR 0004 superseded.
- [ ] Update the domain model to make the observer user-visible and read-only.
- [ ] Describe manual reapply honestly and retain official-entry/SQLite safety claims.
- [ ] Assign one consistent next release version across root npm, Cargo, Tauri, and installer identity.
- [ ] Run all gates before packaging.
- [ ] Build the unsigned current-user NSIS artifact; do not launch or install it.
- [ ] Record exact path, byte count, and SHA-256 locally.

**Verification:**

```powershell
npm run check
npm run qa:theme-mock
npm run tauri build -- --bundles nsis
Get-FileHash "src-tauri\target\release\bundle\nsis\*.exe" -Algorithm SHA256
rg -n "Smart Routing|Codex（主题版）|launch-themed-codex" src src-tauri shared README.md CONTEXT.md docs/adr
```

**Acceptance:**

- [ ] Full repository quality gate and Mock gate pass.
- [ ] One version-consistent local installer exists with recorded proof.
- [ ] Current official ChatGPT/Codex window was not closed or restarted.
- [ ] Public website and download remain unchanged.

---

## T10 — Separately Authorized Real Acceptance and Public Release

**Status:** Blocked by T09 and explicit user authorization

**Objective:** Validate the local installer in a disposable official-app session and publish only the exact accepted artifact.

**Preconditions:**

- [ ] User identifies a safe official-app window/session with no unfinished work.
- [ ] User explicitly authorizes installation and real restart testing.
- [ ] Installer path, byte count, and SHA-256 from T09 are recorded.
- [ ] Current task-bearing window is excluded from termination and restart targets.

**Acceptance sequence:**

- [ ] Capture official shortcut, startup, scheduled-task, process, and SQLite-health baselines read-only.
- [ ] Install the local artifact.
- [ ] Confirm the official entry and official data paths are unchanged.
- [ ] Confirm there is no alternate entry, tray, startup item, watcher, supervisor, or auto-relauncher.
- [ ] Spawn or observe a bounded native subagent and compare its displayed effective model/source with authoritative local metadata.
- [ ] Apply representative abstract, character, dark, light, and local-import themes.
- [ ] Exercise home, task, settings, account, plugin, dialog, menu, dropdown, image, icon, input, send, focus, scroll, and link behavior.
- [ ] Confirm active work blocks normal restart and force flow requires a second current confirmation.
- [ ] Fully close the disposable official session and confirm it remains closed.
- [ ] Reopen through the unchanged official entry and confirm the saved selection requires another explicit apply.
- [ ] Restore official appearance and confirm only owned theme state is removed.
- [ ] Recheck official SQLite health and shortcut/startup baselines.

**Release steps after acceptance:**

- [ ] Copy the exact accepted installer to the website download directory.
- [ ] Synchronize version, byte count, and SHA-256 proof deterministically.
- [ ] Update website copy to describe both read-only observer and one-click themes without routing claims.
- [ ] Run website tests and production build.
- [ ] Publish only after a separate deployment authorization if required by the hosting workflow.
- [ ] Re-download the public installer and compare its size and SHA-256 with the accepted local artifact.

**Verification:**

```powershell
npm run check
npm run qa:theme-mock
npm --prefix website test
npm --prefix website run build
Get-FileHash "website\public\downloads\*.exe" -Algorithm SHA256
```

Also retain sanitized before/after evidence for official shortcut identity, startup registrations, process ownership, observer model provenance, theme interaction checks, restore state, and SQLite health. Do not retain task content, credentials, cookies, or full private database paths.

**Acceptance:**

- [ ] Real observer and theme behavior match the source specification.
- [ ] Official functionality, entry, data, and SQLite remain intact.
- [ ] Public bytes exactly match the accepted local installer.
- [ ] Work record contains only verified outcomes and no credentials or private content.

## Final Completion Checklist

- [ ] T01–T09 are complete and independently verified.
- [ ] T10 either completed under explicit authorization or remains clearly blocked without weakening T01–T09 completion claims.
- [ ] No test was disabled, assertion weakened, or permission broadened to obtain green status.
- [ ] No Smart Routing artifact or observer control action is shipped.
- [ ] No theme can obscure semantic content or capture pointer events.
- [ ] Current official task window was preserved throughout local implementation.
