# ADR 0002: Native routing and CDP control layer

- Status: Accepted in principle; pending written-spec review
- Date: 2026-07-18

## Context

The original product is an external metadata-only observer. The next product iteration must let a user enable quality-first lower-cost subagent routing for the current conversation, keep routed agents in Codex's native subagent panel, expose savings estimates, and add one-click themes.

Codex supports custom native agents with model configuration and bounded nesting, but actual model eligibility depends on the current host, account, and native spawn surface. The current product cannot truthfully inject an external execution into the native agent tree. Codex also has no supported arbitrary UI-extension surface for a per-conversation routing button or theme picker.

The selected Dream Skin reference applies themes without modifying official binaries by using a loopback Chromium DevTools Protocol endpoint. This is update-sensitive and exposes a same-user local-process attachment risk while active.

## Decision

Preserve ADR 0001 as the invariant for the Observer subsystem. Add two opt-in subsystems:

1. A native-agent configurator and routing skill that use only Codex's built-in subagent mechanism. The app installs versioned custom-agent profiles, sets bounded nesting, performs native capability preflight, and routes only profiles whose requested and effective native model match.
2. A hardened CDP control layer that injects a clearly branded routing toggle and declarative theme UI into the existing Codex window. It binds routing to the visible root conversation and never patches official files.

The routing workflow is quality-first: opportunistic delegation, bounded fan-out, implementer self-verification, independent specification-and-quality review, and automatic repair or model escalation. Detached executors and simulated native cards are prohibited.

The theme engine ports or vendors an audited subset of Codex Dream Skin pinned by commit, preserves MIT attribution, accepts only declarative rights-cleared packs, and fails closed on compatibility or signature errors.

## Consequences

- The product as a whole is no longer purely read-only: the configurator writes narrowly scoped, backed-up Codex configuration and the CDP layer changes only live presentation. The Observer remains read-only and metadata-only.
- Initial setup requires one Codex restart. Normal routing and theme switching do not open a second execution window.
- Restoring the official appearance removes only theme-owned scripts and theme preference state. It does not disable root-thread routing or interrupt native subagents; the verified CDP session may remain while another control-layer capability still needs it.
- Restart lifecycle operations are serialized. Safe restart remains the default; an active native child requires a user-confirmed, short-lived, single-use destructive ticket before the verified process tree can be stopped and, after a five-second grace period, terminated leaf-first. This is the only exception to the idle-only rule.
- UI controls can temporarily become unavailable after Codex updates; compatibility fallback preserves official appearance and the Observer.
- Luna and Spark availability cannot be promised ahead of native preflight. Unsupported profiles remain visible as unavailable rather than being silently substituted.
- `agents.max_depth = 2` enables the requested Terra-to-lower-tier delegation but requires stricter application-level fan-out and escalation budgets.
- CDP provides a practical no-binary-patch integration but carries a disclosed loopback same-user risk and ongoing compatibility maintenance.
- A metadata-only MCP sidecar is needed for per-root routing policy and metrics, but it never executes models or receives task content.
- Public theme distribution requires a rights manifest and commercial redistribution review for every asset.

## Alternatives considered

### Detached Luna/Spark executor

Rejected because it would create a separate execution context that does not appear as a real native child under the current conversation.

### Skill and config only

Retained as compatibility fallback. It is safer and more stable but cannot provide the requested one-click per-conversation control or integrated theme picker.

### Patch `app.asar` or WindowsApps

Rejected because it modifies official files, risks signatures and updates, and weakens recovery and trust.

### Simulate native subagent cards through CDP

Rejected because presentation must reflect actual Codex lineage and effective-model metadata.
