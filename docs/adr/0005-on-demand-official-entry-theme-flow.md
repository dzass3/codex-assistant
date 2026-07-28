# ADR 0005: On-demand themes through the official ChatGPT/Codex entry

- Status: Accepted
- Date: 2026-07-22

## Context

The alternate `Codex（主题版）` entry made theme restoration appear automatic, but it changed the user's launch model and created a second visible application path. The accepted product boundary is stricter: the official Microsoft Store ChatGPT/Codex entry remains the only Codex entry, Codex Assistant does not remain resident, and the user decides both when Codex starts and when a theme is applied.

## Decision

Codex Assistant stores the selected theme preference but applies a theme only after an explicit action in Codex Assistant. If the official app is stopped, that action may launch the official AppUserModelID once and apply the selection. If the official app is already running without a verified theme session, Codex Assistant presents the restart impact and waits for explicit confirmation before a guarded restart.

The installer does not create an alternate Codex shortcut, startup entry, tray process, watcher, supervisor, scheduled task, or `Run` value. Upgrades remove only the retired `Codex（主题版）` shortcut when its target is the installed Codex Assistant binary. Official package files, shortcuts, application data, and SQLite databases are outside Codex Assistant ownership.

Ordinary startup and status polling do not apply a saved theme and do not inspect, rewrite or delete `.codex` configuration, agent profiles, MCP entries or global Skills. The only startup migration moves known theme preference, control-session and local-theme entries between Codex Assistant-owned state directories, leaving every unrelated legacy file in place.

## Consequences

- Theme selection survives Codex Assistant and Windows restarts.
- The applied visual theme does not automatically survive a full close and ordinary reopen of official ChatGPT/Codex.
- After an ordinary reopen, the user returns to Codex Assistant and clicks `应用主题` again.
- Switching themes inside a currently verified session does not require another Codex restart.
- This decision supersedes ADR 0004 where ADR 0004 specifies `Codex（主题版）`, automatic reapplication, or runtime cleanup of Codex configuration and Skills.
