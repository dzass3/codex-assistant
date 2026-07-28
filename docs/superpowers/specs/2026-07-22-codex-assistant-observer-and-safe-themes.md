# Codex Assistant — Read-only Subagent Observer and Safe One-click Themes

- Status: Requirements approved; implementation pending
- Date: 2026-07-22
- Product: Codex Assistant for Windows
- Update: bundled-theme count, compatibility adapters, monitor freshness and monitor time semantics are superseded by `2026-07-27-codex-assistant-compatibility-catalog-and-monitor-correctness.md`.

## Summary

Codex Assistant will again expose a read-only real-time subagent observer while retaining and improving its one-click theme manager. The observer and the theme manager are separate product surfaces with a shared safety boundary: monitoring never controls Codex, and theme operations never overwrite official content, configuration, binaries, shortcuts, or databases.

The observer restores the useful part of the original product: a live hierarchy of root tasks and native subagents, actual effective models, reasoning effort, lifecycle state, model drift, and source health. Smart Routing, model assignment, task injection, agent control, and routing configuration remain retired.

The theme manager takes visual and interaction inspiration from [Fei-Away/Codex-Dream-Skin](https://github.com/Fei-Away/Codex-Dream-Skin), but ports only the safety-compatible concepts: a true background layer, native interactive controls, layered translucent surfaces, local theme switching, and exact restore. It does not copy broad selectors, resident tray behavior, automatic relaunch, unverified assets, or any behavior that can obscure or recolor official semantic content.

## Product Decisions

The following decisions were explicitly confirmed:

1. Restore a read-only `实时代理` page; do not restore Smart Routing.
2. Expose two top-level pages: `实时代理` and `一键换肤`.
3. Remember the last selected Codex Assistant page locally; default first launch to `一键换肤`.
4. Show active work by default, allow ended/idle work to be revealed, and create no new activity-history database.
5. Distinguish requested model from authoritative effective model and disclose the data source.
6. Use Codex-Dream-Skin as a safety-filtered design and implementation reference, not as an unreviewed code drop.
7. Preserve the official ChatGPT/Codex entry. After a complete official-app reopen, the user manually clicks `应用主题` again.
8. Apply rich visuals only to compatible main/task pages; keep utility and sensitive pages official or lightly themed.
9. Block a normal restart while native agent work is active; require a current second confirmation for a force restart.
10. Apply one safety contract to every bundled theme in the current catalog specification and every local theme with no exceptions.
11. Complete source, automated tests, and a local Windows installer before runtime acceptance.
12. Do not update the public installer until a separate, safe official-app session passes real acceptance.

## Goals

- Show root and descendant native agents as a live, cycle-safe hierarchy.
- Show effective model, requested model, reasoning effort, lifecycle state, freshness, and sanitized provenance.
- Make model drift and uncertain model identity explicit.
- Preserve metadata-only, read-only observation with no transcript or tool-content access.
- Provide one-click theme application after local environment readiness is established.
- Create a visible background hierarchy, main visual, sidebar material, card material, and composer material without replacing native controls.
- Preserve official text, images, icons, focus behavior, input, menus, buttons, scrolling, and navigation.
- Fail closed and restore the previous verified state after incompatibility or partial application.
- Produce an installable Windows package that is safe to test on another device.

## Non-goals

- Smart Routing, model selection, delegated execution, routing profiles, preflight agents, or per-task routing controls.
- Starting, stopping, interrupting, messaging, or otherwise controlling agents from the observer.
- Displaying prompts, responses, reasoning text, tool calls, command output, patches, file contents, credentials, quotas, or full private paths.
- Modifying `.codex/config.toml`, agent definitions, Skills, MCP configuration, official ChatGPT/Codex files, WindowsApps, `app.asar`, code signatures, shortcuts, or SQLite databases.
- A tray process, startup entry, scheduled task, watcher, supervisor, automatic relauncher, or alternate `Codex（主题版）` entry.
- Automatic theme persistence across a complete official-app process restart.
- Importing concept screenshots that already contain a fake UI.
- Redistributing person, IP, portrait, or third-party assets without an explicit rights record.

## Information Architecture

### Top-level navigation

The application header contains a two-item tab list:

- `实时代理`
- `一键换肤`

There is no Smart Routing tab, button, status chip, onboarding message, IPC command, packaged routing asset, or website claim.

The last selected tab is stored only in Codex Assistant-owned local UI state. If no preference exists, the application opens on `一键换肤`.

### Real-time agent page

The page contains:

- an active-work summary;
- state-database and rollout-observer health;
- an active/all toggle;
- optional model, source, and project-basename filters;
- a manual refresh action;
- a hierarchical root/subagent tree;
- an empty state that explains how native subagents appear;
- a bounded local `CODEX_HOME` override when automatic detection is insufficient.

The root row provides hierarchy context. Descendant rows are visibly labeled `子代理`. Neither type exposes control actions.

### One-click theme page

The page retains:

- local environment readiness checks;
- bundled theme cards;
- device-only image import;
- selection and application state;
- explicit `应用主题` action;
- explicit guarded session start/restart actions;
- exact `恢复官方外观` action;
- clear manual-reapply disclosure.

Theme cards are previews, not screenshots pasted over the official application.

## Observer Data Contract

### Data sources

The Rust backend uses only:

- `state_5.sqlite`, opened read-only, for the initial graph and safe fallback metadata;
- rollout JSONL files, incrementally read through a strict whitelist, for authoritative model and lifecycle metadata.

The observer never reads `auth.json` and does not require a network request.

### Allowed frontend fields

Each observation may contain only:

- thread ID and parent thread ID;
- sanitized agent path or display label;
- display name, role, originator, and project basename;
- requested model;
- effective model;
- model-source classification;
- reasoning effort;
- lifecycle status;
- model-drift boolean;
- subagent boolean and depth;
- started, updated, and freshness timestamps.

No raw rollout record, database row, prompt, response, tool payload, or full workspace path may cross the Tauri boundary.

### Truthfulness rules

1. The child thread's latest `turn_context.model` is the authoritative effective model.
2. A database model is a labeled fallback only.
3. A spawn request model is requested intent only.
4. Requested intent must never be presented as the effective model.
5. If effective identity cannot be confirmed, display `尚未确认`.
6. If requested and effective models differ, display `模型漂移` and both values.
7. Source labels are `运行确认`, `状态库`, `仅请求值`, or `未知来源`.

### Lifecycle states

- `启动中`
- `运行中`
- `可继续调用`
- `已中断`
- `跟踪异常`

Active-first mode includes starting, running, and tracking-error rows plus their ancestors. The optional all mode adds idle and interrupted rows. Snapshots remain in memory and are reconstructed from Codex-owned read-only metadata after restart.

### Health and degradation

- A missing or incompatible database degrades to rollout-only mode where possible.
- A malformed rollout line is discarded and increments only a sanitized error counter.
- A truncated or rotated rollout is rescanned through the whitelist.
- The last valid snapshot remains visible while a source is temporarily degraded.
- Health messages contain no private path or raw record content.

## Theme Architecture

### Reference boundary

The selected Codex-Dream-Skin concepts are:

- loopback CDP attachment to a verified local official process;
- a real background layer below native UI;
- native sidebar, cards, project controls, and composer remaining interactive;
- user-selected local imagery;
- theme switching and exact restoration;
- no official binary modification.

The following concepts are rejected for this product:

- broad selectors that recolor semantic foregrounds or SVG fills;
- decorative pseudo-elements above content;
- resident tray supervision or automatic relaunch;
- automatic theme mutation during startup or polling;
- unverified person/IP assets or UI-bearing concept screenshots;
- attachment to an ambiguous, non-loopback, wrong-user, or non-official process.

### Page classification

Every candidate page is classified before theme application:

1. `main-task`: compatible home or conversation surface; eligible for full theme treatment.
2. `utility`: settings, account, plugin management, browser, or similar functional page; eligible only for a light backdrop or official appearance.
3. `sensitive`: login, authorization, permission, recovery, or security prompt; official appearance only.
4. `unknown`: unrecognized DOM; no theme application.

A compatible main page must contain a visible main surface, sidebar when expected, and a visible, hit-testable composer or the appropriate home-state equivalent.

### Theme ownership

- A session owns at most one `style[data-codex-assistant-theme]` element and one namespaced runtime API.
- Applying a new theme destroys the previous owned runtime before committing the replacement.
- Restore removes only Codex Assistant-owned live nodes and preference state requested by the user.
- No official DOM node is deleted, replaced, cloned, or assigned application semantics by the theme engine.

### CSS safety contract

Every bundled and imported theme must satisfy all of the following:

- backgrounds and decorative layers sit below content;
- decorative layers use `pointer-events: none`;
- semantic text colors are not overridden;
- content images and their sources are unchanged;
- SVG/icon `fill`, `stroke`, and current-color ownership remain native;
- primary-action foreground and fill remain native;
- focus outlines, selection, caret, textarea behavior, and keyboard navigation remain native;
- popovers, menus, dialogs, dropdowns, permission prompts, and tooltips keep native stacking and foregrounds;
- no global `*`, `body *`, generic `svg`, generic `button`, or semantic-token recoloring rule is permitted;
- theme selectors target verified surface containers and presentation-only background/border properties;
- background opacity preserves readable contrast without painting over content.

### Visual quality contract

For compatible main/task pages, every bundled theme must provide:

- a visible main visual with intentional positioning and safe focal area;
- a distinct but quiet sidebar material;
- a restrained header treatment;
- legible card and user-bubble materials;
- one coherent composer glass surface without an opaque inner shell;
- consistent borders, radii, shadows, and accent treatment;
- reduced interference on long task pages compared with the home page.

Visual richness never overrides the CSS safety contract.

### Local imports

- Accept only validated raster image MIME types and bounded encoded sizes.
- Verify the decoded bytes, MIME signature, dimensions, and content hash.
- Store imported assets only in Codex Assistant's device-local state.
- Derive a safe presentation layer without changing the source image.
- Reject UI screenshots, malformed data URLs, unsupported formats, oversized assets, and hash mismatches.
- Never include local imports in the installer, website, telemetry, or logs.

## Theme Operation Flows

### Apply in a verified session

1. User selects a theme.
2. User clicks `应用主题`.
3. Codex Assistant verifies package identity, process ownership, loopback endpoint, page identity, and compatible main DOM.
4. The engine applies one owned style.
5. Verification checks visible background, surfaces, semantic preservation, and hit testing.
6. Only a fully verified result is reported as applied.
7. A failure restores the previous verified theme or official appearance and reports one actionable reason.

### Official app is not running

The current explicit user action may launch the official AppUserModelID once. Codex Assistant does not create or use an alternate entry and does not remain as a supervisor.

### Official app needs a guarded restart

1. The observer refreshes active native work.
2. If active work is zero, the UI requests explicit restart confirmation when required.
3. If active work is non-zero or health is uncertain, normal restart is blocked.
4. A force path requires a fresh impact preview and second confirmation.
5. Identity, process-tree, and active-work changes invalidate the confirmation.
6. Old process descendants must fully exit before official activation.
7. A replacement session is accepted only after stable official UI and app-server verification.

### Complete official-app reopen

The selected theme preference remains saved. Applied CSS does not survive the official process lifetime. After an ordinary full reopen, the user returns to Codex Assistant and clicks `应用主题` again. Polling, monitor refresh, Windows login, and Codex startup never apply a theme automatically.

### Restore

`恢复官方外观` removes the owned style and runtime API and clears the selected preference only as specified by the current restore contract. It never modifies official files, application data, or unrelated Codex Assistant state.

## Cross-feature Rules

- Observer refresh remains read-only regardless of the active page.
- Theme polling and observer polling must not trigger a mutation.
- Active-work counts used by restart protection come from the same reconciled observer snapshot displayed to the user.
- A degraded observer may reduce convenience but must increase restart caution.
- Theme errors must not stop the observer, and observer-source errors must not erase the selected theme.
- Navigation state, monitor settings, theme state, and imported assets use separate owned keys and bounded directories.

## IPC and Permission Boundary

The public Tauri command surface may contain:

- observer snapshot, refresh, settings, and bounded local source selection;
- theme snapshot, environment report, preview, local import, session start, apply, restore, and explicit restart confirmation.

It must not contain routing, agent creation, follow-up, interruption, model mutation, content retrieval, arbitrary filesystem, arbitrary shell, or arbitrary CDP-evaluation commands.

Tauri permissions, generated handlers, frontend APIs, shared types, and product-identity tests must remain synchronized.

## Testing Requirements

### Observer tests

- Read-only SQLite projection and byte-identical fixture verification.
- Whitelist rejection of prompts, responses, reasoning, tool arguments, and tool output.
- Requested/effective precedence and model drift.
- Lifecycle transitions including follow-up turns and interruption.
- Cycle-safe root/subagent hierarchy.
- Active-only filtering with ancestor retention.
- Health degradation and last-good snapshot behavior.
- Strict frontend parsing of unknown or malformed payloads.
- No routing button, routing type, routing command, routing resource, or routing copy.

### Theme engine tests

- All bundled packs pass rights and asset-integrity gates.
- Every generated CSS source excludes semantic foreground and generic icon selectors.
- Only one owned style exists after apply and after repeated switches.
- Failed application and failed switching never report the new theme as applied.
- Restore removes only owned state.
- Utility, sensitive, and unknown pages remain official or use the allowed light treatment.
- Local import rejects spoofing, oversize, malformed bytes, UI screenshots where detectable, and hash mismatch.

### Playwright Mock contract

The isolated Mock ChatGPT/Codex harness must exercise every bundled theme and representative local-import output. For each theme it must assert:

- semantic text content and color are unchanged;
- content-image source and decoded dimensions are unchanged;
- SVG fill is unchanged;
- primary-action text and fill are unchanged;
- text, image, sidebar, icon button, primary action, composer, and input are hit-testable;
- clicking sidebar/buttons/icons works;
- typing and sending works;
- switching keeps exactly one owned style;
- restore matches the visual baseline for owned properties.

The harness must retain a deliberate overlay canary that fails with a covering-layer assertion, proving the suite can detect the target regression.

### Full quality gate

Before packaging:

```powershell
npm run check
npm run qa:theme-mock
```

Formatting, lint, type checking, frontend tests, Rust unit/integration tests, theme Mock tests, and product-identity scans must all pass.

## Runtime Acceptance and Release Gate

The implementation phase may build a local unsigned NSIS installer, but it must not install it, close the current official task window, or update the public website automatically.

Public release requires a separately authorized, disposable official-app session that verifies:

- the official ChatGPT/Codex entry is unchanged;
- no alternate shortcut, tray, startup entry, scheduled task, watcher, or auto-relauncher exists;
- observer rows reflect a real native subagent and its actual model source;
- themes do not impair home, task, settings, account, plugin, dialog, menu, image, icon, input, send, focus, scroll, or link behavior;
- active work blocks an ordinary restart;
- manual reapply after a full official-app reopen behaves honestly;
- restore returns to official appearance;
- official SQLite data remains healthy and untouched.

Only after that acceptance may the exact installer bytes, size, SHA-256, website copy, and download target be synchronized and published.

## Implementation Surface

Expected observer restoration work includes:

- restore `shared/monitor-types.ts` without routing types;
- restore a strict `src/lib/monitorApi.ts` and `src/hooks/useMonitor.ts`;
- restore monitor components with all routing controls removed;
- add a two-page navigation component with no routing option;
- expose only monitor read commands/events from Rust/Tauri;
- synchronize permissions and tests;
- update `CONTEXT.md`, current README claims, and ADR status so observer visibility is no longer described as retired.

Expected theme work includes:

- formal page classification;
- safer surface selectors and layered materials;
- expanded bundled/local theme Mock coverage;
- regression tests for utility and sensitive pages;
- environment and restart guidance that incorporates observer health and active work;
- release packaging without alternate entry or background behavior.

## Acceptance Criteria

- The application exposes exactly two top-level product pages: `实时代理` and `一键换肤`.
- The first launch opens themes; subsequent launches restore the last selected page.
- A real or synthetic native subagent appears under its root with truthful model provenance.
- No monitor screen or payload contains conversation/tool content or a full private path.
- No Smart Routing UI, API, resource, command, profile, or product claim is reachable or shipped.
- Every bundled theme in the current catalog specification and representative local imports pass the semantic-preservation and interaction contract.
- Unknown or incompatible official pages remain unthemed.
- Active native work blocks normal restart and force restart requires a fresh second confirmation.
- Complete official-app restart never causes automatic theme application.
- Restore affects only Codex Assistant-owned theme state.
- `npm run check` and the full Mock theme harness pass before the installer is built.
- A local installer is produced, but public distribution waits for separately authorized real acceptance.

## Open Questions

None. Product-scope decisions are approved; implementation and runtime acceptance remain pending.
