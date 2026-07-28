# ADR 0004: Theme-only Codex Assistant

- Status: Superseded in product-surface scope by ADR 0006; launch, persistence and runtime-cleanup sections superseded by ADR 0005
- Date: 2026-07-21

## Context

Codex Assistant had accumulated observation, model-routing and UI-injected routing capabilities alongside one-click themes. Those capabilities increased setup risk, altered Codex configuration, and made it harder to guarantee that a theme would never interfere with normal Codex use. The product requirement is now singular: apply and restore visual themes safely on supported Windows Codex installations.

## Decision

At the time of this decision Codex Assistant exposed only theme management. ADR 0006 later restores a user-visible, strictly read-only observer while preserving this ADR's removal of Smart Routing, agent controls and routing resources.

Themes are applied to verified Microsoft Store Codex processes through a random loopback CDP endpoint. The engine owns one marked style element, never modifies the official package, never changes semantic foreground or icon colors, and verifies the main task surface and composer before committing an apply operation. Bundled assets must pass the rights manifest gate. User imports remain device-only.

The application exposes a local environment report before theme operations. Microsoft Store Codex is activated through `IApplicationActivationManager` with the exact AppUserModelID rather than by executing a protected WindowsApps path. The current-user installer creates `Codex（主题版）`, a normal Start menu shortcut that runs a short-lived apply launcher. The launcher never registers for login startup, remains in the tray, watches Codex or relaunches it after the user closes it. This launch mechanism is retained as historical context and is not shipped from version 0.10.0 onward.

Upgrade cleanup was designed to remove exact Codex Assistant-owned routing profiles, MCP configuration and files while migrating theme preference and local theme assets. This runtime cleanup mechanism is retained as historical context and is not executed from version 0.10.0 onward; ADR 0005 sets the narrower ownership boundary.

## Consequences

- A first themed session may require an explicit Codex restart; switching themes in that verified session does not.
- Historical note: the retired `Codex（主题版）` reapply behavior was superseded by ADR 0005 and is not shipped.
- An ordinary already-running Codex is reported as requiring an explicit themed restart; it is never silently replaced by the shortcut.
- Closing Codex is always user-controlled. Codex Assistant does not supervise, relaunch or patch Codex at login.
- Failed compatibility checks restore or retain the official appearance rather than leaving a partial theme.
- Historical routing plans and ADRs remain as design history but are superseded and are not shipped in the installer.
