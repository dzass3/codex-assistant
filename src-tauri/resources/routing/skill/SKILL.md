---
name: codex-assistant-smart-routing
description: Quality-first native subagent routing for Codex Assistant
---

# Native smart routing

Use only real native Codex children below the visible root conversation. Never run `codex exec`, open another execution window, create hidden tasks, or simulate native cards. Before every spawn, query eligibility and use a profile only when the requested and effective native model have been proved equal for the exact direct or nested route. Asset presence is not eligibility; honestly leave unavailable profiles unused.

Classify before delegation. Keep architectural, security, destructive, deployment, credential, ambiguous, or restricted work in the Sol/root path. Use Spark only for fully specified mechanical low-risk work, Luna for bounded low-risk work, and Terra for cross-layer work, always subject to eligibility and quality floors. Prefer quality over quota or latency.

At most three routed children may be active per root, only one depth-two child may be active, and no subtask may receive more than two automatic escalations. Never create recursive reviewer fan-out. For profile/model overrides, spawn with `fork_turns="none"` or an explicitly bounded recent-history fork; never override a profile with full-history inheritance.

Every implementer self-verifies. Trigger independent specification-and-quality review when quality requires it for high-risk or complex code-changing work; ordinary low-risk tasks do not require a second review by default. Repair and re-review failures; escalate within the stated budget when repair cannot meet the quality bar. Spark and Luna must not delegate further. Only Terra may spawn one lower-tier native child within the stated budget. Report uncertainty and unavailable capability truthfully.

The routing metadata MCP is metadata-only. Never send prompts, task content, responses, reasoning, commands, patches, paths, credentials, or tool input/output to it. Use only its enabled metadata tools. See [policy](references/policy.md) for the routing contract.
