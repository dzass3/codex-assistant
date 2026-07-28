# Codex Assistant — Windows Theme Compatibility, 14-theme Catalog and Monitor Correctness

- Status: Requirements approved; implementation pending
- Date: 2026-07-27
- Product: Codex Assistant for Windows
- Supersedes: the bundled-theme count, selector-only compatibility assumptions, observer freshness and observer time semantics in the 2026-07-22 specification

## Summary

Codex Assistant retains exactly two product surfaces: `实时代理` and `一键换肤`. This increment replaces the original 12 bundled themes with 14 redistribution-approved themes, makes theme compatibility explicit across supported Windows architectures and official Store builds, and corrects monitor refresh and time semantics.

The implementation must remain safe by construction. It runs as the current user, never edits the official package or ChatGPT/Codex data, preserves official semantic content and controls, and never reports a theme as applied unless the visible primary task page passes post-apply semantic and interaction verification.

## Confirmed Product Decisions

1. Support Windows 10 22H2 and Windows 11 with separate x64 and ARM64 installers.
2. Support only the official Microsoft Store ChatGPT/Codex desktop application. Web, PWA, portable and third-party packages are unsupported.
3. Preserve the official ChatGPT/Codex entry, explicit apply action and manual reapply boundary from ADR 0005.
4. Replace selector-only page detection with version-adaptive, multi-evidence compatibility adapters.
5. Rich-theme only compatible home, project and task pages. Login, account, payment, authorization, permission, security and recovery pages remain official.
6. Ignore utility, DevTools, preload, background and invisible targets when determining whether the visible primary task page applied successfully.
7. Remove all 12 previous bundled themes and ship only the 14 approved themes in this specification.
8. Retain device-only local image import as a separate catalog source.
9. Clear a missing legacy bundled-theme preference, restore official appearance and ask the user to choose again. Never silently map an old theme ID to a new image.
10. Give each bundled theme an immutable English ID and a Chinese display name.
11. Calibrate focal point, mask, contrast and component tokens separately for every bundled theme.
12. Do not require administrator privileges and do not modify official files, package data, SQLite databases or user content.
13. Correct observer refresh to a typical 300–500 ms and a 95th-percentile target within one second.
14. Bound live agent state to the current official Codex process session and derive displayed age from last observed activity.
15. Keep current parent/child, model, lifecycle and descendant-count meanings except where stale liveness must be corrected.
16. Claim an architecture as supported only after its clean-environment release gate passes.

## Goals

- Make one-click theme application honest and actionable on supported Windows hosts.
- Keep ChatGPT/Codex text, images, icons, controls, input, focus, navigation and semantic states fully usable.
- Ship 14 coherent offline themes whose background, sidebar, cards, composer and secondary controls belong to the same visual system.
- Detect official application changes without relying on one brittle selector set.
- Show native-agent changes quickly while keeping idle CPU usage near one percent or lower.
- Prevent historical unclosed events from appearing as current green running work.
- Preserve exact, owned restoration and protect official application data during install, use, upgrade and uninstall.

## Non-goals

- Smart Routing, agent control, task injection or model mutation.
- A replacement or alternate ChatGPT/Codex launcher.
- Background residency, tray supervision, startup registration or automatic relaunch.
- Automatic theme application after an ordinary complete official-app reopen.
- Broad best-effort theming of unknown official builds.
- Modification of official application binaries, resources, databases, shortcuts or package registration.
- Redistribution of a device-local imported image.
- AI redraw, outpainting or visual replacement of the 14 approved source images.

## Supported Host Matrix

| Dimension        | Supported                                                | Unsupported                                              |
| ---------------- | -------------------------------------------------------- | -------------------------------------------------------- |
| Operating system | Windows 10 22H2; Windows 11                              | Earlier Windows; non-Windows                             |
| Architecture     | x64; ARM64 through separate installers                   | Other architectures                                      |
| Application      | Official Microsoft Store ChatGPT/Codex                   | Web, PWA, portable, repackaged or third-party builds     |
| Privilege        | Current standard Windows user                            | Administrator-only setup as a requirement                |
| Theme session    | Verified loopback-only session owned by the current user | Ambiguous, remote, wrong-user or non-official CDP target |

The environment report identifies OS build, architecture, exact Store package, application version, visible official window count, verified session reachability, saved preference and compatibility-adapter result. It returns one concrete next action and never collapses incompatible states into a generic apply failure.

Safe conditions that belong to Codex Assistant state may be repaired automatically. If the official application must restart, the existing restart guard refreshes current work and requests one explicit confirmation. Known active work or uncertain current-session evidence blocks a normal restart.

## Bundled Theme Catalog

The source directory contains 14 user-approved PNG assets. The user states that every image was generated, commissioned or otherwise authorized for redistribution with the installer and contains no unauthorized celebrity, brand or third-party IP. Implementation must record this statement per asset in the bundled rights manifest together with source filename, immutable theme ID, SHA-256, rightsholder, redistribution permission and review date.

The sorted source-to-theme assignment is:

| Source file                                    | Stable ID        | Display name |
| ---------------------------------------------- | ---------------- | ------------ |
| `ChatGPT Image 2026年7月24日 11_20_32 (3).png` | `wisteria-bride` | 紫藤花嫁     |
| `ChatGPT Image 2026年7月24日 11_20_32 (4).png` | `mint-gentleman` | 薄荷礼服     |
| `ChatGPT Image 2026年7月24日 11_20_32 (9).png` | `iris-gentleman` | 鸢尾绅士     |
| `ChatGPT Image 2026年7月24日 17_57_29 (2).png` | `crimson-palace` | 赤月华庭     |
| `ChatGPT Image 2026年7月24日 17_57_31 (7).png` | `verdant-fairy`  | 森灵花语     |
| `ChatGPT Image 2026年7月24日 17_57_31 (9).png` | `desert-prince`  | 金沙王庭     |
| `ChatGPT Image 2026年7月24日 18_24_59.png`     | `oasis-prince`   | 绿洲暮光     |
| `ChatGPT Image 2026年7月24日 18_33_41.png`     | `sakura-moon`    | 樱月夜宴     |
| `ChatGPT Image 2026年7月24日 20_02_56.png`     | `seaside-blue`   | 海风晴夏     |
| `ChatGPT Image 2026年7月25日 00_06_10.png`     | `autumn-wuxia`   | 云岭秋侠     |
| `ChatGPT Image 2026年7月25日 00_06_19.png`     | `meteor-evening` | 流星晚霞     |
| `ChatGPT Image 2026年7月25日 00_06_48.png`     | `violet-blade`   | 紫夜剑影     |
| `ChatGPT Image 2026年7月26日 13_38_21.png`     | `fuji-autumn`    | 富士秋光     |
| `ChatGPT Image 2026年7月26日 14_00_18.png`     | `spring-street`  | 春日花街     |

### Asset derivation

- Preserve the original PNGs in the controlled source archive; do not embed the large originals in the runtime catalog.
- Strip metadata and generate a high-quality WebP runtime asset plus a smaller WebP catalog preview.
- Store SHA-256 for source and derived assets and fail the catalog build on mismatch.
- Landscape themes use `cover` with a manually reviewed focal point and safe content area.
- Portrait themes use the same image twice: a blurred full-bleed backdrop and a clear subject layer aligned per image.
- Do not redraw, outpaint or synthesize missing image regions.
- Bundle every runtime asset and preview for offline use. No runtime network fetch is permitted.

### Catalog migration

- Remove the old 12 bundled packs and their distributable assets.
- If a saved selected/applied ID is no longer present, remove only that invalid preference and restore official appearance.
- Show `原主题已下架，请从 14 个新主题中重新选择`.
- Do not map a removed ID to a visually unrelated replacement.
- Keep valid local imported themes and their device-only state untouched.

## Per-theme Visual System

Each bundled theme owns manually reviewed presentation tokens for:

- backdrop focal point, dual-layer layout and overlay;
- main and sidebar surface opacity;
- header, card, user-bubble and composer materials;
- border, radius, blur and shadow;
- non-semantic accent;
- secondary button, tab and icon-button surface;
- hover, pressed, selected and focus-visible states;
- long-task-page attenuation;
- reduced-motion behavior.

The engine may use automated color extraction as a starting measurement, but the shipped values must be reviewed per theme. One generic pink token set is not acceptable.

Theme presentation may style container background, border, radius, shadow and backdrop filters. It must retain official text and icon ownership for primary, stop, permission, delete, danger, success, disabled and other semantic actions.

## Compatibility Adapter Model

### Host fingerprint

The engine builds a bounded fingerprint from:

- exact verified Store package identity and application version;
- page target type, visibility and current-user process ownership;
- structural landmarks;
- ARIA roles and accessible labels;
- visible and hit-testable main capabilities;
- the presence of a task composer or compatible home-state action.

No prompt, response, user content, tool content or private full path is part of the fingerprint.

### Adapter registry

An adapter defines evidence and presentation-only surface locators for a compatible official build family. Evidence uses alternatives and capabilities rather than one exact selector. Sensitive evidence wins before main-page evidence.

Adapter outcomes are:

- `compatible-main`: eligible for full theme transaction;
- `utility`: official appearance or an explicitly allowed light backdrop;
- `sensitive`: official appearance only;
- `unknown-build`: official appearance and actionable incompatibility guidance;
- `invalid-target`: ignored for operation completeness.

The visible, interactive primary task page is the commit target. DevTools, preload, service/background and invisible targets are ignored. More than one visible official application window remains a readiness action rather than an ambiguous apply attempt.

### Transaction and verification

1. Detect the supported host and select a compatible adapter.
2. Prepare one namespaced, owned presentation runtime.
3. Inject only into the primary compatible page.
4. Verify semantic preservation, visible surfaces and hit testing.
5. Commit the selected/applied state only after all required checks pass.
6. On failure, remove the candidate runtime and restore the prior verified theme or official appearance.

An unsupported official version displays `当前官方版本尚未适配` with detected version and one safe next action. It never displays `已应用`.

## Hard Usability and Accessibility Gate

Every bundled theme and representative local-theme output must pass:

- WCAG AA contrast for body text and common controls;
- no hidden, replaced, recolored or covered official text, content image, SVG icon or action;
- preserved official semantic state for primary, stop, permission, destructive, success and disabled controls;
- visible keyboard focus, selection, caret and input;
- hit-testable sidebar navigation, menus, buttons, links, composer and input;
- usable layout at 100%, 125%, 150% and 200% Windows scaling and supported compact window sizes;
- correct stacking for menus, dropdowns, dialogs, tooltips and permission prompts;
- official appearance on login, account, payment, authorization, permission, security and recovery pages.

A failed gate is a failed theme operation, not a warning.

## Observer Refresh Contract

### Trigger model

- Use Windows filesystem notifications for relevant state and rollout changes.
- Coalesce bursts before reading so one logical update does not create redundant full refreshes.
- Retain a one-second polling fallback for missed, unsupported or degraded notifications.
- Manual refresh bypasses the normal wait and reads immediately.
- Emit a frontend snapshot only when the sanitized stable projection changes, except health/freshness transitions which are themselves meaningful changes.

### Targets

- Typical observed change to visible update: 300–500 ms.
- 95th percentile: no more than one second under the supported fixture budget.
- Idle monitor CPU target: approximately one percent or lower on the release reference device.
- A source backlog or missed freshness target changes the user-facing state to `更新延迟`; it must not remain green `实时`.

## Observer Time and Session Semantics

- Backend timestamps are UTC epoch milliseconds.
- Frontend relative age is derived from the latest observed activity in the current Codex process session.
- Frontend full-time tooltips use the Windows locale and time zone.
- Relative labels use seconds, minutes and hours; ages over 24 hours use days.
- A task-start event without later activity or a terminal event cannot remain green running beyond the bounded current-session evidence window.
- Older unclosed records appear only in `全部` as `历史状态未闭合`.
- Ambiguous records in the current session appear as `状态待确认`, block safe restart and return to `运行中` only after new activity.
- Active mode includes current-session starting, running and uncertain rows plus required ancestors.
- When the official application is not running, the monitor says `Codex 未运行` and does not infer live work from history.

The restart guard consumes the same current-session projection shown to the user. Confirmed current work blocks normal restart; uncertain current-session evidence also blocks normal restart.

## Monitor Status Presentation

Connection and freshness are distinct:

| State        | Meaning                                                                           |
| ------------ | --------------------------------------------------------------------------------- |
| `实时`       | Required sources are connected and the projection is within the freshness target  |
| `更新延迟`   | Sources are reachable but the projection is older than the expected refresh bound |
| `状态待确认` | Current-session evidence is incomplete or contradictory                           |
| `监控异常`   | A required source cannot be read; the UI offers an immediate retry                |

Manual refresh rereads evidence; it does not reset, close or manufacture agent state.

## Privacy, Ownership and Uninstall

- Observer reads remain metadata-only and read-only.
- No official package file, WindowsApps executable, application database, user conversation or application shortcut is modified.
- Theme assets, preferences, adapters and session metadata live only in Codex Assistant-owned locations.
- Uninstall removes only Codex Assistant-owned installed files and optional owned state selected by the user.
- Uninstall and upgrade must not open, migrate, repair or delete official ChatGPT/Codex databases.
- Application errors and diagnostics use bounded codes and sanitized labels, never full private paths or raw records.

## Testing Requirements

### Catalog and migration

- Assert exactly 14 bundled theme IDs and no old bundled ID.
- Verify source and derived hashes, MIME, dimensions, size budgets and rights records.
- Verify every runtime and preview asset is included offline.
- Test missing-old-preference migration and preservation of local imports.

### Adapter and page safety

- Maintain sanitized official-page fixtures per supported build family.
- Test alternate structural and ARIA evidence, not only exact selectors.
- Test primary home, project and task states.
- Test utility, sensitive, unknown, DevTools, preload, background and invisible targets.
- Test multiple visible official windows and unsupported official builds.
- Prove incompatible or partially verified pages never commit or report success.

### Theme interaction matrix

Run every 14-theme fixture at representative window sizes and 100%, 125%, 150% and 200% scaling. Assert unchanged semantic text, image source, icon fill/stroke, primary/stop action semantics, input behavior, focus, hit testing and stacking. Retain deliberate overlay, semantic-recolor and invisible-focus canaries that must fail.

### Observer correctness

- Test notification-triggered refresh, burst coalescing, one-second fallback and manual refresh.
- Measure latency distribution and idle CPU on the reference fixture budget.
- Test current-session boundary, historical unclosed state and ambiguous current evidence.
- Test UTC-to-Windows-local rendering, day rollover and future/invalid timestamp handling.
- Regression-test parent/child structure, effective model provenance, lifecycle labels and descendant counts.
- Test that the restart guard and visible projection use the same session-bounded evidence.

### Full local gate

```powershell
npm run check
npm run qa:theme-mock
```

The implementation plan may add explicit catalog-generation, adapter-fixture and performance-test commands; they become mandatory release gates when added.

## Packaging and Release Gate

Produce separate x64 and ARM64 installers. An architecture may be labeled supported only after:

1. clean standard-user installation on Windows 10 22H2 and Windows 11;
2. official Store application detection and version reporting;
3. first apply, theme switch, restore and manual reapply after ordinary reopen;
4. main-page, menu, dialog, settings and sensitive-page regression;
5. local-import isolation;
6. uninstall verification showing official application and data remain healthy;
7. exact installer SHA-256 and architecture-specific download metadata.

If a real-device gate has not passed, label that architecture `待验证` and do not claim compatibility. Public website and installer replacement remain a separate release action after the exact bytes pass acceptance.

## Acceptance Criteria

- The product still exposes exactly `实时代理` and `一键换肤`.
- The bundled catalog contains only the 14 approved themes with the IDs and display names in this specification.
- Device-local imports remain usable and undistributed.
- An unsupported official build remains official and never reports theme success.
- A supported primary task page receives the selected theme without changing or covering official semantic content or controls.
- All 14 themes pass the usability, scaling and interaction matrix.
- Legacy bundled-theme preferences fail safely without remapping or affecting local imports.
- Monitor updates normally within 300–500 ms and meets the one-second 95th-percentile target.
- Historical unclosed tasks are not green current work; ambiguous current evidence is visibly uncertain and restart-conservative.
- No normal operation, upgrade or uninstall changes the official package or local database.
- x64 or ARM64 support is claimed only after the corresponding clean-device release gate passes.

## Open Questions

None. Requirements are approved; implementation planning and execution remain pending.
