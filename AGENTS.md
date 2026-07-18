# Codex Agent Monitor contributor guide

## Commands

```powershell
npm run tauri dev
npm run check
npm run tauri build -- --bundles nsis
```

After code or configuration changes, add proportionate tests and run `npm run check` before committing.

## Architecture and privacy invariants

- Rust/Tauri backend: `src-tauri/src/monitor/`.
- React UI: `src/App.tsx`, `src/components/`, and `src/hooks/useMonitor.ts`.
- Shared frontend contract: `shared/monitor-types.ts`.
- Open Codex SQLite only with read-only flags.
- Parse rollout records through an explicit metadata whitelist.
- Never retain, log, emit, serialize, or display prompts, responses, reasoning text, tool arguments, tool output, credentials, or full workspace paths.
- `turn_context.model` is the authoritative effective model; database model is fallback; requested-only values must be labeled as such.
- Keep the Tauri command surface and permissions minimal and synchronized.
