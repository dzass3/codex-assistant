# Codex Assistant — Domain Context

## Glossary

### Root thread

A user-visible Codex task that owns zero or more descendant agent threads.

### Agent thread

A Codex execution thread with its own identity, model settings, lifecycle events, and optional parent thread.

### Subagent

An agent thread created by another agent thread. A subagent remains the same agent even when it moves between running and idle turns.

### Native subagent

A subagent created through Codex's built-in agent mechanism and represented in Codex's native agent tree. A routed native subagent must satisfy this definition; an external process or detached conversation never does.

### Delegation mode

A user-enabled policy attached to the currently selected root thread. While enabled, later native subagent work in that conversation may be routed repeatedly to an eligible model according to task complexity. Enabling delegation mode does not itself start a run and must not launch a detached Codex process or a second conversation window.

### Model router

The quality-first policy used by the current Codex conversation to classify each proposed subtask and select an eligible native custom-agent profile for GPT-5.3 Codex Spark, GPT-5.6 Luna, Terra, or Sol. It optimizes quota usage and elapsed time only after the required quality, capability, tool, and risk constraints are satisfied.

### Model route

The selected execution model plus the recorded reasons for choosing it, including task complexity, risk, required capabilities, estimated cost, and any quality evidence.

### Native capability preflight

A one-time, non-destructive verification performed after custom-agent installation and Codex restart. It confirms which configured model profiles the current Codex host can actually spawn as native children and observe in the native subagent panel.

### Eligible model

A configured model profile that passed native capability preflight for the current Codex version, profile version, requested model, and route depth. Presence in the root model picker alone is not proof that a model is eligible for native subagent routing.

### Routed native subagent

A model-selected child created through Codex's built-in subagent mechanism. A conversation may create multiple routed native subagents over time; each must appear in the current conversation's native subagent panel with its effective model.

### Quality escalation

Re-routing or retrying a subtask on a more capable model when its initial route lacks confidence, fails validation, exceeds its repair budget, or encounters work outside its approved complexity and risk boundary.

### Routing tier

One of four default quality bands: Spark for fully specified mechanical work, Luna for clear low-risk bounded work, Terra for cross-file or judgment-heavy work, and Sol for ambiguous, architectural, high-risk, or final whole-task review work. Luna and Spark tiers exist only when their profiles are eligible models.

### Quality gate

The evidence required before routed work can be accepted: implementer self-verification followed by an independent native review for specification compliance and code quality. A failed gate triggers repair and re-review or quality escalation; it never becomes an accepted result merely because it was cheaper or faster.

### Task envelope

The minimum information supplied by the parent conversation to one routed native subagent: task instructions, relevant project context, constraints, sandbox policy, and output contract. It is not part of an observation.

### Delegation activity

The user-visible state of delegation for a root thread, including whether delegation mode is enabled and whether a routed native subagent is queued, running, completed, failed, or cancelled.

### Opportunistic delegation

The rule that delegation mode permits but does not force native subagent creation. The parent handles trivial work directly and delegates only when the expected benefit of specialization, parallelism, or a lower-cost eligible model exceeds the coordination overhead while preserving the quality requirement.

### Counterfactual savings estimate

An explicitly labelled estimate of the time and quota that the same work might have consumed without delegation mode. It is not a measured saving unless both strategies were actually run under comparable conditions.

### Routing baseline

The quality-matched historical distribution of models, elapsed time, and available usage signals observed while delegation mode was disabled. It is used to estimate the counterfactual cost of comparable work and carries a sample count and confidence level.

### Theme pack

A selectable Codex appearance bundle containing visual assets, style parameters, attribution, compatibility metadata, and a rights manifest. A packaged theme is distributable product content; a user-imported local theme is not.

### Rights manifest

The auditable record attached to every bundled theme asset, including source, author or rights holder, license or written authorization, commercial redistribution scope, required attribution, and review status.

### Theme rights gate

The release rule that permits bundled distribution only when every asset in a theme pack has verifiable commercial redistribution rights. Unverified celebrity likenesses, protected characters, third-party artwork, and repository preview screenshots fail this gate and may only be supplied locally by the user.

### Theme engine

The audited local runtime that applies themes to the official Codex UI through Chromium DevTools Protocol without modifying the official package, `app.asar`, WindowsApps files, or code signature.

### Themed session

A Codex desktop session launched once with a random loopback-only CDP endpoint and verified official process identity. Theme changes may be applied live inside that existing window until the user restores the official appearance or the session ends.

### Conversation control layer

The Codex Assistant control surface injected into the active Codex composer through the verified theme engine. It binds routing state to the visible root thread, displays delegation activity, and contributes only a compact routing control marker to each submitted user turn while enabled; it does not read or retain the user's prompt body.

### Compatibility fallback

The safe degraded mode used when Codex custom-agent preflight or CDP UI compatibility fails. Native routing is limited to eligible profiles, theme injection is disabled when unsafe, and the user receives a clear status instead of a simulated capability or modification of official application files.

### Requested model

The model explicitly supplied when the parent asks Codex to create a subagent. It records intent and can be absent when inheritance is requested.

### Effective model

The model recorded in the subagent's latest `turn_context`. This is the authoritative answer to “which model is this subagent actually using?” and can differ from the requested model.

### Reasoning effort

The effective reasoning-strength setting recorded for the subagent's latest turn.

### Observation

A metadata-only view of one agent thread at a point in time. It contains identity, lineage, model, reasoning effort, lifecycle state, timestamps, and provenance, but no conversation or tool content.

### Observation freshness

The age of the newest trusted metadata event used to construct an observation.

### Lifecycle state

One of `starting`, `running`, `idle`, `interrupted`, or `tracking-error`. `Idle` means the most recent turn completed and the agent may still receive a follow-up; it does not mean the agent was destroyed.

### Model drift

A condition where the requested model and effective model are both known and differ.

### Metadata-only boundary

The observer rule that conversation bodies, reasoning text, tool arguments, tool outputs, full filesystem paths, authentication material, and secrets are never retained, logged, sent to the observer frontend, or displayed. Delegation mode changes the parent conversation's native routing policy but does not create a second content-processing path inside Codex Assistant.

## Invariants

- Effective model wins over requested model whenever both exist.
- A requested model without an effective model is displayed as pending confirmation, not as actual usage.
- Lifecycle state is derived from task and activity events; the persisted spawn-edge `open` value is not treated as proof that an agent is running.
- All Codex data sources are opened read-only.
- Project identity is shown as a directory basename unless the user explicitly reveals the full path.
- Monitoring failure must never alter Codex state.
- Delegation mode is opt-in for a selected root thread and must remain visibly active until disabled or the thread ends.
- Delegation mode uses opportunistic delegation; it must not create a subagent for every user message or every trivial operation.
- Before routing, the parent considers delegation overhead and keeps work local when a child is unlikely to produce a net time or quota benefit.
- Delegation mode must not implement model execution through a detached process, hidden conversation, injected imitation card, or second execution window.
- Every routed execution must be a real native subagent associated with the current root thread.
- Installing or changing native custom-agent profiles may require one Codex restart; after preflight, normal delegation-mode activation must not open or restart another window.
- The model router may select only eligible models confirmed by native capability preflight.
- An unsupported configured model must be shown as unavailable and must never be silently substituted or visually imitated.
- Required quality is a hard routing constraint; quota and elapsed time are secondary optimization objectives.
- GPT-5.3 Codex Spark and GPT-5.6 Luna may handle only work inside their declared capability and risk boundaries.
- Every routed native subagent records its model route and exposes the routing reason to the user.
- The default routing tiers are Spark for fully specified mechanical work, Luna for clear low-risk bounded work, Terra for cross-file or judgment-heavy work, and Sol for architecture, high-risk work, and final whole-task review.
- Routed implementation requires self-verification plus an independent specification-and-quality review before acceptance.
- A failed quality gate must be repaired and re-reviewed or escalated to a more capable eligible model.
- Savings shown without a controlled comparison run must be labelled as estimates, with their basis and confidence visible.
- Savings estimates compare against a quality-matched routing baseline and include all failed attempts, repair turns, review runs, and escalation costs.
- When the baseline sample is insufficient, the product shows that evidence is insufficient instead of presenting a precise saving.
- The product must not duplicate a real task solely to manufacture an enabled-versus-disabled comparison.
- Every bundled theme pack must pass the theme rights gate and ship with its rights manifest and required attribution.
- User-imported local theme assets are not uploaded, published, or redistributed by Codex Assistant.
- Enabling the theme engine may restart Codex once; subsequent theme switches operate in the existing Codex window and must not open a second Codex window.
- Safe restart never terminates active native agents. A user-confirmed force restart is a destructive exception: it uses a 60-second single-use ticket bound to the exact verified root process identity and current impact, offers a five-second cancellable grace period, terminates only the revalidated descendant tree leaf-first, and never retries after an irreversible partial failure.
- The theme engine uses a random loopback-only CDP port, verifies official Codex process identity, and closes the endpoint when restoring the official appearance.
- Codex updates trigger a compatibility check; an incompatible or failed theme must fall back to the official appearance without modifying official application files.
- The selected architecture is a native-agent configurator plus a conversation control layer; detached execution and official application patching are excluded.
- The conversation control layer binds delegation mode to the visible root thread and must not enable routing for another task in the same workspace.
- Compatibility fallback preserves official appearance and exposes unsupported models or controls explicitly rather than simulating them.
