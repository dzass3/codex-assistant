# Mandatory native-subagent orchestration for complex work

- Status: Superseded by ADR 0004; no longer part of Codex Assistant
- Date superseded: 2026-07-21

When delegation mode is enabled, a complex task with at least two independently deliverable and verifiable work units is decomposed into two to four bounded packages executed by real native Codex subagents. The root agent remains the orchestrator and final decision-maker; at most three children run concurrently, and a fourth package waits for capacity. We chose mandatory fan-out over opportunistic delegation so the mode has a predictable user-visible meaning, while restricted, destructive, credential-bearing, indivisible, or otherwise unsafe work remains in the root or Sol path.
