# Codex Assistant native routing policy

Route only verified native children. Eligibility is keyed by Codex version, profile version, requested model, route kind, and depth. Requested/effective-model drift is unavailable, not a substitute.

Quality gates take precedence over cost and time. Maintain the three-child, one-nested-child, and two-escalation limits. Each implementation validates itself with focused tests. Request an independent reviewer only when quality requires it for high-risk or complex code-changing work; ordinary bounded changes use the same direct TDD verification flow. Repair and re-review failures or use bounded escalation when the quality bar is not met.

The metadata MCP receives opaque IDs, enums, timestamps, counters, and booleans only. It must never receive conversation or tool content. Create one opaque `subtask_id` for a logical unit of work and reuse that ID across replacement child attempts so the server can enforce escalation counts 0, 1, and 2. Use a new `child_thread_id` for every native child, record the returned escalation count, and finish that child with `routing_quality_record` before starting its replacement.
