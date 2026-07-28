# Codex Assistant runtime QA inventory

## Sign-off claims

- A bundled theme can be selected from Codex Assistant and becomes visibly applied in the active official Codex main task window.
- Restoring the official appearance removes only Codex Assistant-owned theme styles and leaves Smart Routing configuration unchanged.
- Smart Routing can be enabled for the selected root task, mounts its control in that task's active Codex composer, and exposes the configured metadata MCP tools to a restarted Codex session.
- The Arina pink theme appears in the local installed catalog, applies through the same verified flow, and is excluded from distributable release assets.

## Controls and state transitions

- Themes navigation: themes list -> selected/pending -> verified applied or actionable failure.
- Theme card action: official Codex -> themed session -> different theme in the same verified session.
- Restore official appearance: applied theme -> official appearance without changing root routing policy.
- Smart Routing navigation and root toggle: disabled -> pending/open-task-needed -> verified enabled -> disabled.
- Codex composer control: absent -> mounted for the matching root -> enabled/disabled with the same policy state.

## Functional evidence

- Use normal pointer/keyboard input in the installed Codex Assistant UI.
- Verify backend-visible theme state and computed style in the active Codex main task renderer.
- Verify routing config assets, MCP `tools/list`, root identity binding, and exact composer marker insertion.
- Exercise one full reversible theme cycle and one full reversible routing cycle.

## Visual evidence

- Capture Codex Assistant initial Themes and Smart Routing pages at the launched size.
- Capture Codex after a verified bundled-theme application and after local Arina application.
- Inspect status copy, selection indicators, disabled/loading states, clipping, overflow, contrast, and accidental duplicate windows.
- Repeat at a smaller realistic Codex Assistant window size.

## Exploratory cases

- Codex is already open without a Codex Assistant-owned CDP session.
- A main task exists alongside utility/secondary renderer targets.
- The selected root task is not currently open, then is opened.
- A previously active external theme session is absent but has recoverable archived state.
- Restart is blocked by an active task and must fail closed without claiming success.

## Verified runtime — 2026-07-20

- Installed build: Codex Assistant `0.7.1` at `D:\Software\Codex Agent Monitor\codex-assistant.exe` (PID `43256` during final acceptance).
- Verified official host: Codex `26.715.4045.0` at PID `43792`; CDP listeners were loopback-only (`127.0.0.1:60263` for Codex and `127.0.0.1:9229` for Codex Assistant).
- Theme result: session `ready`, selected/applied theme `arina-pink`; the Codex renderer exposed theme id `arina-pink`, a `919432`-byte injected style, a real image data URL, readable pink surfaces, and no horizontal or vertical viewport overflow at `2562 × 1394`.
- Catalog result: `Arina 粉晶花园` appeared as `仅限本地导入` / `仅当前设备` and `当前主题`; the source image remained in the local application-data theme directory and was not added to distributable source assets.
- Native preflight result: direct Terra, Spark, Luna, and Sol plus nested Luna and Spark all reached `eligible` with exact effective-model matches on Codex `26.715.4045.0`.
- Routing result: root `019f7362…` reached `enabled`; the matching Codex composer control reported `aria-pressed="true"` and `Codex Assistant · Enabled` for the same opaque route key.
- Privacy result: preflight projection accepted only the exact four native profile names when `requested_model` was absent; arbitrary role/originator content was not copied into the preflight observation or eligibility state.
- Visual/exploratory pass: exercised theme-session recovery, local-theme current state, sequential direct/nested preflight, a blocked toggle retry, final enablement, long Smart Routing page scrolling, and the fixed Codex composer at native window size. No horizontal clipping, duplicate control, unreadable text, or accidental second Codex root window was observed.

## Final evidence

- `outputs/assistant-smart-routing-complete-final.png`
- `outputs/assistant-smart-routing-enabled-final.png`
- `outputs/assistant-arina-current-final.png`
- `outputs/codex-arina-routing-enabled-final.png`

## Regression coverage added

- A different preflight attempt can be inserted after the previous directive is cleared, while duplicate insertion of the same directive remains blocked.
- The native-child timeout window begins when that directive is actually inserted, not when the entire sequential preflight starts.
- A known native profile role supplies the requested-model intent when current rollout metadata omits `requested_model`; the effective model must still come from authoritative `turn_context` metadata.

## Verified theme fidelity — 2026-07-20 (`0.7.2`)

- Reinstalled Codex Assistant `0.7.2`, recovered the verified theme session, and explicitly re-enabled Smart Routing for root `019f7362…`; the Codex composer badge returned to `Codex Assistant · Enabled` with route key `4eb2d5ec…`.
- Applied all 12 bundled themes through their real Theme Management card buttons, then restored the local `arina-pink` theme. Every bundled theme resolved to a data-image backdrop, `0.48` main alpha, `2px` main blur, `0.72` sidebar alpha, `22px` composer radius, palette border and shadow.
- Effective backdrop visibility measured `0.416` for every image theme, above the `0.4` verification floor. The main visual remains recognizable while the sidebar and composer retain stronger glass layers.
- Primary-action contrast passed for every bundled palette; the lowest live result was Crimson Relay at `5.58:1`. Button token icons matched the computed action foreground. Arina user cards measured `18px` radius with palette border and shadow.
- Full `npm run check` passed after the final clarity/contrast changes: 12 Vitest files / 82 tests and the complete Rust suite, including 11 theme contracts and the local encoded-budget boundary.
- Routed diagnosis used an authoritative `gpt-5.6-luna` child (`medium`). Independent review attempts used authoritative `gpt-5.6-terra` children (`high`) and exposed the encoded-size, generic-button-token, near-black contrast, and main-blur issues before final runtime acceptance.
- Final evidence: `outputs/theme-qa-all-bundled-v0.7.2.png`, `outputs/theme-qa-reference-vs-arina-v0.7.2.png`, `outputs/theme-qa-composer-reference-vs-arina-v0.7.2.png`, `outputs/theme-qa-arina-v0.7.2.png`, and `outputs/assistant-themes-v0.7.2.png`.
