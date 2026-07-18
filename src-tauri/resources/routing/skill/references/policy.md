# Codex Assistant native routing policy

Route only verified native children. Eligibility is keyed by Codex version, profile version, requested model, route kind, and depth. Requested/effective-model drift is unavailable, not a substitute.

Quality gates take precedence over cost and time. Maintain the three-child, one-nested-child, and two-escalation limits. Each implementation validates itself; code changes require an independent reviewer, followed by repair and re-review or bounded escalation.

The metadata MCP receives opaque IDs, enums, timestamps, counters, and booleans only. It must never receive conversation or tool content.
