# Codex Assistant domain context

## Product boundary

Codex Assistant is a Windows read-only native-agent observer and safe theme manager for the official Microsoft Store Codex desktop app. It has exactly two user-facing pages: `实时代理` and `一键换肤`. Model routing, preflight orchestration and injected task controls are retired product concepts.

The observer exposes only bounded local metadata: opaque task identity, parent/child relationships, status, project/role labels, requested and authoritative effective model, reasoning effort, freshness and source health. It never exposes prompts, responses, reasoning, tool input/output or full private paths.

## Terms

### Read-only observer

A user-visible projection of Codex's existing local state database and rollout metadata. It starts in active-only mode, preserves required ancestors, can reveal idle/interrupted rows on request, and never creates, routes, interrupts or follows up an agent.

### Effective model provenance

The authoritative model observed from turn context or state metadata, shown separately from a requested model. Requested-only intent is labelled `尚未确认` and is never promoted into an effective model value.

### Observer health

A bounded healthy/degraded/error status for the state database and rollout source. Raw backend errors and full paths are replaced with fixed user-facing copy. Degraded or tracking-error health makes restart safety uncertain.

### Current Codex process session

The lifetime of the currently verified official Codex process tree. Only activity supported by evidence in this lifetime may be presented as currently running.

### Observed activity time

The most recent whitelisted state or rollout evidence associated with an agent in the current Codex process session. It is distinct from task creation time and is the source of the user-facing relative time.

### Historical unclosed state

An older start record that has no matching terminal record and no activity evidence in the current Codex process session. It remains available as history but is never presented as live work.

### Theme catalog

The union of bundled themes that passed the redistribution rights gate and local themes imported by the current Windows user. Catalog presence does not prove a theme is currently visible.

### Bundled theme

A declarative pack shipped with the installer. Its visual asset, palette, effects, attribution and rights metadata are listed in the shared catalog and bundled rights manifest.

### Local theme

A user-owned PNG, JPEG or WebP image imported on one device after signature, MIME, dimensions, byte budget and SHA-256 validation. It is never published or redistributed by Codex Assistant.

### Theme preference

The theme selected for a future or current verified session. A saved preference can be paused and is not evidence that Codex is themed.

### Verified theme session

A Codex process launched with a random loopback-only CDP endpoint after official Store package, executable, process owner and listener identity have been verified. The session record contains only bounded identity metadata and expires when identity changes.

### Applied theme

A preference whose owned style has been injected into every compatible main Codex task page and independently verified. Only this state may be presented as success.

### Paused theme

A saved preference with no currently verified theme session. Resuming may require the user to start a theme session and explicitly approve a Codex restart.

### Theme environment report

A local, user-visible readiness classification derived from platform support, the exact Store package, verified Codex window count, reachable theme session and saved preference. It reports one bounded next action instead of a generic apply failure.

### Supported theme host

A Windows 10 22H2 or Windows 11 x64/ARM64 environment with the official Microsoft Store ChatGPT/Codex application and a release-validated compatibility adapter. Web, PWA, portable and third-party packages are outside this boundary.

### Theme compatibility adapter

A version-tolerant set of structural and semantic evidence used to classify an official page and locate presentation-only surfaces. An adapter fails closed when the evidence is incomplete or unknown.

### Primary task page

The visible, interactive official home, project or task page selected from a verified single-window session. Its successful semantic and interaction verification determines whether a theme operation may commit.

### Theme welcome surface

A presentation-owned empty-state layer shown only on a compatible primary task page that has a native composer and no visible conversation. It contains four bounded shortcuts that either invoke an already visible matching official action or prefill the native composer; it never sends a message, replaces a native control or remains visible after conversation evidence appears.

### Theme reading surface

A bounded translucent material applied only to one official message, tool result, file result, code block or important status unit. It improves contrast without becoming a page-wide wash, changing semantic media, or capturing interaction outside the official unit.

### Manual reapply boundary

The selected preference persists, while applied CSS belongs to the current verified session and is re-created only by a later explicit apply. A normal full reopen through the official entry does not automatically apply a theme.

### Official appearance

Codex without Codex Assistant-owned style and script nodes. Restore removes only owned nodes and clears the saved preference; it never edits official application files.

### Theme transaction

The prepare, inject, verify and commit sequence used for apply and switch operations. A failure before commit leaves or restores a consistent prior/official state. Partial success is never reported as applied.

### Restart guard

The active-work and confidence projection derived from the same snapshot shown by the observer. A safe restart is blocked for known active work or uncertain monitor health. A force restart requires a 60-second, single-use ticket bound to the exact process identity, active count and confidence, a cancellable grace period and leaf-first revalidation.

### Theme-state migration

A bounded migration that moves only known theme preference, control-session and local-theme entries between Codex Assistant-owned state directories. It does not inspect or mutate `.codex`, global Skills, official application data or unrelated legacy files.

## Invariants

- The desktop UI exposes exactly `实时代理` and `一键换肤`; the observer side is read-only and the theme side mutates only Codex Assistant-owned theme state and verified live presentation.
- No shipped entrypoint accepts a routing sidecar command and no routing runtime asset is bundled.
- Observer payloads and UI never contain conversation/tool content or full private paths.
- Official Codex package files are never modified.
- CDP is loopback-only and bound to one verified current-user official Codex process tree.
- The generated theme never changes conversation content foreground, primary-action fill or semantic SVG fill. It may set inherited foreground color only inside verified navigation, header, output-panel and composer chrome so dark glass remains readable.
- Theme backgrounds and decorative layers never capture pointer events or sit above Codex content.
- Main content and sidebar must have visible bounds; an existing composer must remain visible and hit-testable.
- A theme switch is committed only after every compatible main page verifies; utility pages may remain official.
- Failed or incompatible injection falls back to official appearance without claiming success.
- Bundled assets must pass the rights gate; local imports stay device-only.
- Safe restart never stops known or uncertain native work, waits for the complete original verified process tree to exit, and refuses to activate a replacement while any owned Store runtime remains. Force restart is explicit, short-lived, identity-and-impact-bound and non-retrying after irreversible partial failure.
- Codex starts only through the official entry or a current explicit Codex Assistant action; a normal full reopen never triggers automatic theme application. Codex Assistant is not a supervisor, tray keeper or auto-relauncher.
- Startup and status polling are read-only outside Codex Assistant's own state directory: they never apply a saved theme, rewrite `.codex/config.toml`, or delete agent, MCP or Skill files.
- Store activation uses the exact official AppUserModelID and a bounded argument string; the protected WindowsApps executable is never launched directly. A replacement is not accepted until one exact, direct official `codex.exe` app-server remains stable across the readiness window.
- Restoring official appearance deletes only Codex Assistant-owned live nodes and theme preference state.
