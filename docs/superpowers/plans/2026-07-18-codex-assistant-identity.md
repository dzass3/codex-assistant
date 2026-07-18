# Codex Assistant Identity and Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the installed Windows product from Codex Agent Monitor `0.4.0` to Codex Assistant `0.5.0` while preserving its installer identity, existing settings location, observer behavior, and in-place upgrade path.

**Architecture:** Keep the Tauri bundle identifier and current settings directory as stable internal migration anchors. Rename the Rust/Node package and binary, centralize user-facing brand copy in the React config module, and add cross-file identity regression tests before changing production values.

**Tech Stack:** Tauri 2, Rust 2021, React 19, TypeScript 7, Vitest 4, npm, Cargo, Windows NSIS.

## Global Constraints

- Product display name is exactly `Codex Assistant`.
- Release version is exactly `0.5.0`.
- Preserve Tauri identifier exactly as `com.codexagentmonitor.desktop`.
- Preserve the existing settings directory name `codex-agent-monitor`; it is an internal compatibility path and must not be renamed in this plan.
- Preserve all Observer behavior, read-only Codex access, event name `monitor://snapshot`, and metadata-only frontend contract.
- Do not add routing, MCP, CDP, theme, quota, or network behavior in this plan.
- Do not edit the separate `site/` repository in this plan.
- Do not remove or overwrite user settings.
- Every code/config change is test-first and each task ends with a focused commit.

## File structure

- `src/config.ts`: single frontend source for product name, tagline, event name, and refresh interval.
- `src/config.test.ts`: frontend identity contract.
- `src/App.tsx`: consumes shared product copy rather than duplicating brand strings.
- `src/App.test.tsx`: verifies the rendered name/tagline without using real Tauri IPC.
- `index.html`: pre-React document title.
- `src-tauri/tauri.conf.json`: Windows bundle display name/version/title while preserving identifier.
- `src-tauri/Cargo.toml`: Rust package, binary, library, and release version.
- `src-tauri/src/main.rs`: references the renamed library crate.
- `src-tauri/src/lib.rs`: updated panic/launch product context only; command behavior is unchanged.
- `src-tauri/tests/product_identity.rs`: cross-file regression guard for version, identifier, binary, title, and legacy settings path.
- `package.json` / `package-lock.json`: Node package name and version.
- `README.md`, `AGENTS.md`, `CHANGELOG.md`: user and contributor documentation for the new identity and compatibility promise.

---

### Task 1: Lock the cross-file product identity

**Files:**
- Create: `src-tauri/tests/product_identity.rs`
- Modify: `src/config.test.ts`
- Modify: `src/config.ts`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/Cargo.lock`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `package.json`
- Modify: `package-lock.json`

**Interfaces:**
- Produces frontend constants `PRODUCT_NAME`, `PRODUCT_TAGLINE`, `MONITOR_EVENT`, and `DEFAULT_REFRESH_MS` from `src/config.ts`.
- Produces Rust library crate `codex_assistant_lib` and binary `codex-assistant`.
- Preserves Tauri application identifier `com.codexagentmonitor.desktop` and monitor event `monitor://snapshot` for later tasks.

- [ ] **Step 1: Extend the failing frontend identity test**

Replace `src/config.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import {
  DEFAULT_REFRESH_MS,
  MONITOR_EVENT,
  PRODUCT_NAME,
  PRODUCT_TAGLINE,
} from "./config";

describe("product configuration", () => {
  it("uses the Codex Assistant identity and stable monitor event", () => {
    expect(PRODUCT_NAME).toBe("Codex Assistant");
    expect(PRODUCT_TAGLINE).toBe("原生代理路由、模型观察与主题管理");
    expect(MONITOR_EVENT).toBe("monitor://snapshot");
    expect(DEFAULT_REFRESH_MS).toBe(1000);
  });
});
```

- [ ] **Step 2: Add the failing Rust cross-file identity guard**

Create `src-tauri/tests/product_identity.rs`:

```rust
const TAURI_CONF: &str = include_str!("../tauri.conf.json");
const CARGO_TOML: &str = include_str!("../Cargo.toml");
const MAIN_RS: &str = include_str!("../src/main.rs");
const RUNTIME_RS: &str = include_str!("../src/monitor/runtime.rs");

#[test]
fn release_identity_is_consistent_and_upgrade_safe() {
    assert!(TAURI_CONF.contains(r#"\"productName\": \"Codex Assistant\""#));
    assert!(TAURI_CONF.contains(r#"\"version\": \"0.5.0\""#));
    assert!(TAURI_CONF.contains(r#"\"identifier\": \"com.codexagentmonitor.desktop\""#));
    assert!(TAURI_CONF.contains(r#"\"title\": \"Codex Assistant\""#));
    assert!(CARGO_TOML.contains("name = \"codex-assistant\""));
    assert!(CARGO_TOML.contains("name = \"codex_assistant_lib\""));
    assert!(CARGO_TOML.contains("version = \"0.5.0\""));
    assert!(MAIN_RS.contains("codex_assistant_lib::run()"));
    assert!(RUNTIME_RS.contains("const SETTINGS_DIRECTORY: &str = \"codex-agent-monitor\";"));
}
```

- [ ] **Step 3: Run focused tests and verify they fail for the old identity**

Run:

```powershell
npx vitest run src/config.test.ts
cargo test --manifest-path src-tauri/Cargo.toml --test product_identity
```

Expected: the frontend test fails because `PRODUCT_TAGLINE` is missing/old name remains; the Rust test fails because Tauri/Cargo still contain the `0.4.0` monitor identity.

- [ ] **Step 4: Implement the frontend identity constants**

Set `src/config.ts` to:

```ts
export const PRODUCT_NAME = "Codex Assistant";
export const PRODUCT_TAGLINE = "原生代理路由、模型观察与主题管理";
export const MONITOR_EVENT = "monitor://snapshot";
export const DEFAULT_REFRESH_MS = 1000;
```

- [ ] **Step 5: Rename package and Tauri identities while preserving the bundle identifier**

Apply these exact values:

```json
// package.json fields
{
  "name": "codex-assistant",
  "version": "0.5.0"
}
```

```toml
# src-tauri/Cargo.toml fields
[package]
name = "codex-assistant"
version = "0.5.0"

[[bin]]
name = "codex-assistant"

[lib]
name = "codex_assistant_lib"
```

```json
// src-tauri/tauri.conf.json fields
{
  "productName": "Codex Assistant",
  "version": "0.5.0",
  "identifier": "com.codexagentmonitor.desktop",
  "app": { "windows": [{ "title": "Codex Assistant" }] }
}
```

Update `src-tauri/src/main.rs` to call:

```rust
codex_assistant_lib::run()
```

Change the final `expect` text in `src-tauri/src/lib.rs` to:

```rust
.expect("error while running Codex Assistant");
```

Run `npm install --package-lock-only` and `cargo check --manifest-path src-tauri/Cargo.toml` to regenerate only the lockfile identity entries.

- [ ] **Step 6: Run focused identity tests**

Run:

```powershell
npx vitest run src/config.test.ts
cargo test --manifest-path src-tauri/Cargo.toml --test product_identity
```

Expected: both test commands pass.

- [ ] **Step 7: Commit the identity contract**

```powershell
git add src/config.ts src/config.test.ts src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/main.rs src-tauri/src/lib.rs src-tauri/tests/product_identity.rs package.json package-lock.json
git commit -m "refactor: rename application to Codex Assistant"
```

### Task 2: Render and document the new product identity

**Files:**
- Create: `src/App.test.tsx`
- Modify: `src/App.tsx`
- Modify: `index.html`
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CHANGELOG.md`

**Interfaces:**
- Consumes `PRODUCT_NAME` and `PRODUCT_TAGLINE` from `src/config.ts`.
- Preserves the existing `useMonitor()` contract and all Observer UI behavior.
- Produces the visible Codex Assistant brand copy consumed by release packaging and the later public-site plan.

- [ ] **Step 1: Write the failing application brand test**

Create `src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("./hooks/useMonitor", () => ({
  useMonitor: () => ({
    snapshot: null,
    connected: false,
    refreshing: false,
    loading: false,
    error: null,
    settings: null,
    refresh: vi.fn(),
    setCodexHome: vi.fn(),
  }),
}));

describe("App identity", () => {
  it("renders the Codex Assistant name and product scope", () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "Codex Assistant" })).toBeInTheDocument();
    expect(screen.getByText("原生代理路由、模型观察与主题管理")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the focused test and verify the old hard-coded heading fails**

Run:

```powershell
npx vitest run src/App.test.tsx
```

Expected: FAIL because the app still renders `Codex Agent Monitor` and the old subtitle.

- [ ] **Step 3: Consume centralized brand copy in the application and HTML title**

In `src/App.tsx`, import the constants:

```ts
import { PRODUCT_NAME, PRODUCT_TAGLINE } from "./config";
```

Render them in the brand block:

```tsx
<h1>{PRODUCT_NAME}</h1>
<p>{PRODUCT_TAGLINE}</p>
```

Change `index.html` to:

```html
<title>Codex Assistant</title>
```

Do not alter monitor cards, filters, health, tree, IPC, or privacy copy.

- [ ] **Step 4: Update user and contributor documentation**

Make these exact documentation changes:

- `README.md`: title `# Codex Assistant`; describe the current Live Agents observer as the implemented `0.5.0` foundation; state that smart routing and themes are separately gated upcoming modules; preserve every privacy guarantee and development command.
- `AGENTS.md`: title `# Codex Assistant contributor guide`; keep all existing architecture/privacy invariants verbatim.
- `CHANGELOG.md`: prepend `## 0.5.0 — 2026-07-18` with bullets for the Codex Assistant rename, preserved upgrade identity/settings, and unchanged read-only Observer; do not claim routing or themes are implemented.

- [ ] **Step 5: Run frontend and full repository checks**

Run:

```powershell
npx vitest run src/App.test.tsx src/config.test.ts
npm run check
```

Expected: focused tests pass; TypeScript, lint, formatting, Clippy, Rust formatting, all Vitest tests, and all Cargo tests pass.

- [ ] **Step 6: Build the release and NSIS installer**

Run:

```powershell
npm run build
npm run tauri build -- --bundles nsis
```

Expected: the web build succeeds; Tauri creates a `0.5.0` Codex Assistant NSIS installer while retaining identifier `com.codexagentmonitor.desktop`. Record the exact artifact path and size in the task report; do not install it during this task.

- [ ] **Step 7: Commit the visible identity and documentation**

```powershell
git add src/App.tsx src/App.test.tsx index.html README.md AGENTS.md CHANGELOG.md
git commit -m "docs: present the Codex Assistant identity"
```

## Plan acceptance

- All `0.5.0` product surfaces say Codex Assistant.
- The package and binary are renamed, but Tauri identifier and the existing settings directory remain stable.
- The monitor event and Observer contracts are byte-for-byte compatible where required.
- No routing, MCP, CDP, theme, network, or quota behavior is introduced.
- `npm run check`, frontend build, and NSIS release build pass.
- An upgrade installer artifact is produced for later smoke testing and public release work.
