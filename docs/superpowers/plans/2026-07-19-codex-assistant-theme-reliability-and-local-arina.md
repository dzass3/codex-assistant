# Codex Assistant Theme Reliability and Local Arina Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one-click themes visibly correct and truthfully verified, preserve operation errors, and add the user's current Arina skin to the local-only theme catalog without packaging or redistributing it.

**Architecture:** Keep bundled rights-audited packs immutable. Add a strict `LocalThemeCatalog` that reads one-image packs only from Codex Assistant's owned application-data directory, verifies paths, MIME types, size, and SHA-256, and exposes previews through one identifier-only Tauri command. Generate theme CSS from validated bytes, verify the parsed data URL and readable themed surfaces through CDP, and keep failed receipts visible until the next explicit operation.

**Tech Stack:** Rust 1.82, Tauri 2, serde/serde_json, sha2/base64, React 19, TypeScript 7, Vitest/Testing Library, CDP, Playwright Interactive.

## Global Constraints

- Do not modify the official Codex package, `app.asar`, WindowsApps files, or code signature.
- Bundled themes must continue to pass the commercial redistribution rights gate.
- The Arina asset is local-only and must be excluded from Git, installers, release bundles, and published artifacts.
- Theme success means every compatible Codex main task page contains the owned style, the image URL parses as a `data:image/*;base64` URL, and the themed surfaces have the declared readable text color.
- Theme commands accept validated theme identifiers only; the WebView never supplies an arbitrary filesystem path.
- Public TDD seams: `theme_application_source_with_asset`, `LocalThemeCatalog`, `RoutingApplication::theme_snapshot/apply_theme_with`, `toThemeUiSnapshot/themeApi`, and the rendered `ThemesPage`.

---

### Task 1: Correct CSS generation and truthful CDP verification

**Files:**

- Modify: `src-tauri/src/theme.rs`
- Test: `src-tauri/tests/theme_contract.rs`

**Interfaces:**

- Consumes: validated `ThemePack` metadata and the exact verified bytes for its backdrop asset.
- Produces: `pub fn theme_application_source_with_asset(pack: &ThemePack, image_bytes: Option<&[u8]>) -> Result<String, ThemeValidationError>` and a backdrop-sensitive `theme_verification_source(pack)`.

- [ ] **Step 1: Write the failing image URL and verification tests**

```rust
#[test]
fn image_theme_emits_a_parseable_data_url_without_app_protocol_fallback() {
    let pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| matches!(pack.backdrop, ThemeBackdrop::Image { .. }))
        .expect("image theme");
    let source = theme_application_source(&pack).expect("theme source");
    assert!(source.contains("url(\\\"data:image/"));
    assert!(!source.contains("url(\\\\\\\"data:image/"));
    assert!(theme_verification_source(&pack)
        .expect("verification source")
        .contains("data:image/"));
}

#[test]
fn image_theme_rejects_missing_or_hash_mismatched_runtime_bytes() {
    let pack = bundled_theme_packs()
        .into_iter()
        .find(|pack| matches!(pack.backdrop, ThemeBackdrop::Image { .. }))
        .expect("image theme");
    assert_eq!(
        theme_application_source_with_asset(&pack, Some(b"wrong")),
        Err(ThemeValidationError::InvalidAsset)
    );
}
```

- [ ] **Step 2: Run the focused Rust test and verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test theme_contract image_theme_ -- --nocapture`

Expected: FAIL because the emitted script contains an escaped `url(\\\"data:...)` payload and `theme_application_source_with_asset` does not exist.

- [ ] **Step 3: Implement byte verification, valid CSS quoting, and readable token overrides**

```rust
pub fn theme_application_source_with_asset(
    pack: &ThemePack,
    image_bytes: Option<&[u8]>,
) -> Result<String, ThemeValidationError> {
    validate_theme_pack(pack, pack.category != ThemeCategory::LocalImport)?;
    let background = match &pack.backdrop {
        ThemeBackdrop::Gradient { angle, colors } => {
            format!("linear-gradient({angle}deg, {}, {}, {})", colors[0], colors[1], colors[2])
        }
        ThemeBackdrop::Image { asset_id, overlay, focal_x, focal_y } => {
            let asset = pack.assets.iter().find(|asset| asset.id == *asset_id)
                .ok_or(ThemeValidationError::InvalidAsset)?;
            let bytes = image_bytes.ok_or(ThemeValidationError::InvalidAsset)?;
            if sha256_hex(bytes) != asset.sha256.to_ascii_lowercase() {
                return Err(ThemeValidationError::InvalidAsset);
            }
            let encoded = STANDARD.encode(bytes);
            format!(
                "linear-gradient({overlay}99, {overlay}99), url(\"data:{};base64,{encoded}\") {focal_x}% {focal_y}% / cover no-repeat fixed",
                asset.mime_type,
            )
        }
    };
    // Serialize the complete CSS once as a JS string. The CSS itself contains
    // ordinary quotes, never backslash-prefixed quote characters.
    build_theme_script(pack, &background)
}
```

Add scoped selectors for Codex's existing token classes and controls, using the pack palette rather than new hard-coded colors:

```css
main.main-surface [class*="text-token-"],
aside.app-shell-left-panel [class*="text-token-"],
main.main-surface button,
aside.app-shell-left-panel button,
main.main-surface .ProseMirror,
main.main-surface [data-message-author-role] {
  color: var(--codex-assistant-theme-text) !important;
}
```

For image packs, make verification require `backdrop.backgroundImage.includes("data:image/")`, reject `app://`/`file://`, and require the computed main/sidebar color to equal the declared palette text color. Keep gradient verification on `linear-gradient` so it does not require an image.

- [ ] **Step 4: Run the focused tests and verify green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test theme_contract image_theme_ -- --nocapture`

Expected: PASS, with the application source containing a normal quoted data URI and a verification script that cannot accept the malformed `app://-/%22data:` result.

- [ ] **Step 5: Commit the vertical slice**

```bash
git add src-tauri/src/theme.rs src-tauri/tests/theme_contract.rs
git commit -m "fix: verify visible theme assets"
```

### Task 2: Load verified local-only theme packs from application data

**Files:**

- Create: `src-tauri/src/local_theme.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/routing_app.rs`
- Modify: `src-tauri/src/theme.rs`
- Test: `src-tauri/tests/local_theme_catalog.rs`
- Test: `src-tauri/tests/theme_application.rs`

**Interfaces:**

- Consumes: `%APPDATA%/codex-agent-monitor/routing/local-themes/<theme-id>/theme.json` and one `<asset-id>.<jpeg|png|webp>` file.
- Produces: `LocalThemeCatalog::in_directory`, `LocalThemeCatalog::packs`, `LocalThemeCatalog::asset_bytes`, `LocalThemeCatalog::preview_data_url`, and `RoutingApplication::theme_preview_data_url`.

- [ ] **Step 1: Write the failing local catalog contract tests**

```rust
#[test]
fn local_catalog_loads_one_hash_verified_image_pack() {
    let temporary = tempdir().expect("tempdir");
    let bytes = b"local-image";
    write_local_pack(temporary.path(), "arina-pink", bytes, &sha256_hex(bytes));
    let catalog = LocalThemeCatalog::in_directory(temporary.path()).expect("catalog");
    let packs = catalog.packs();
    assert_eq!(packs.len(), 1);
    assert_eq!(packs[0].category, ThemeCategory::LocalImport);
    assert_eq!(packs[0].rights.status, RightsStatus::LocalOnly);
    assert_eq!(catalog.asset_bytes("arina-pink").expect("asset"), bytes);
    assert!(catalog.preview_data_url("arina-pink")
        .expect("preview")
        .starts_with("data:image/jpeg;base64,"));
}

#[test]
fn local_catalog_fails_closed_for_hash_mismatch_and_symlinked_assets() {
    let temporary = tempdir().expect("tempdir");
    write_local_pack(temporary.path(), "arina-pink", b"tampered", &"0".repeat(64));
    let catalog = LocalThemeCatalog::in_directory(temporary.path()).expect("catalog");
    assert!(catalog.packs().is_empty());
    assert!(catalog.asset_bytes("arina-pink").is_none());
}
```

- [ ] **Step 2: Run the local catalog test and verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test local_theme_catalog -- --nocapture`

Expected: FAIL because `local_theme` and `LocalThemeCatalog` do not exist.

- [ ] **Step 3: Implement the strict local catalog**

```rust
pub struct LocalThemeCatalog {
    root: PathBuf,
}

impl LocalThemeCatalog {
    pub fn in_directory(state_directory: &Path) -> Result<Self, String>;
    pub fn packs(&self) -> Vec<ThemePack>;
    pub fn asset_bytes(&self, theme_id: &str) -> Option<Vec<u8>>;
    pub fn preview_data_url(&self, theme_id: &str) -> Option<String>;
}
```

The loader must accept only safe slugs, immediate child directories, regular non-symlink files, one image asset, the exact MIME extension, at most 2 MiB, matching lowercase SHA-256, `category = "local-import"`, `rights.status = "local-only"`, and `commercial_redistribution = false`. Its `preview_path` must equal `local-theme:<theme-id>`. Any malformed pack is omitted rather than partially loaded.

- [ ] **Step 4: Run the local catalog test and verify green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test local_theme_catalog -- --nocapture`

Expected: PASS for the valid fixture and PASS by omission for tampered or linked fixtures.

- [ ] **Step 5: Write the failing application catalog/preference tests**

```rust
#[test]
fn snapshot_and_preferences_accept_a_present_local_pack() {
    let fixture = application_fixture_with_local_theme("arina-pink");
    let app = fixture.app();
    assert!(app.theme_snapshot().packs.iter().any(|pack| pack.id == "arina-pink"));
    let receipt = app.apply_theme_with("arina-pink", |_| Ok(1));
    assert_eq!(receipt.status, OperationStatus::Applied);
    assert_eq!(app.theme_snapshot().selected_theme_id.as_deref(), Some("arina-pink"));
}
```

- [ ] **Step 6: Run the application test and verify red**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test theme_application snapshot_and_preferences_accept_a_present_local_pack -- --nocapture`

Expected: FAIL because snapshots and `ThemePreferenceStore` still accept bundled IDs only.

- [ ] **Step 7: Merge bundled and local catalogs in `RoutingApplication`**

```rust
fn theme_packs(&self) -> Vec<ThemePack> {
    let mut packs = bundled_theme_packs();
    packs.extend(self.local_theme_catalog.packs());
    packs
}

pub fn theme_preview_data_url(&self, theme_id: &str) -> Option<String> {
    self.local_theme_catalog.preview_data_url(theme_id)
}
```

Initialize `LocalThemeCatalog` from the same owned state directory as `ThemePreferenceStore`, validate saved IDs against `theme_packs()`, resolve local bytes before calling `apply_theme_on_pages`, and leave the existing `apply_theme_with(&ThemePack)` seam intact for tests.

- [ ] **Step 8: Run application and catalog tests and verify green**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test local_theme_catalog --test theme_application -- --nocapture`

Expected: PASS; a present valid local pack is selectable and a removed/tampered local pack cannot remain a valid preference.

- [ ] **Step 9: Commit the vertical slice**

```bash
git add src-tauri/src/local_theme.rs src-tauri/src/lib.rs src-tauri/src/routing_app.rs src-tauri/src/theme.rs src-tauri/tests/local_theme_catalog.rs src-tauri/tests/theme_application.rs
git commit -m "feat: load local-only theme packs"
```

### Task 3: Render local previews and keep failures visible

**Files:**

- Modify: `shared/theme-types.ts`
- Modify: `src/lib/themeApi.ts`
- Modify: `src/lib/themeApi.test.ts`
- Modify: `src/hooks/useTheme.ts`
- Modify: `src/hooks/useTheme.test.ts`
- Modify: `src/components/ThemesPage.tsx`
- Modify: `src/components/ThemesPage.test.tsx`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/permissions/default.toml`
- Modify: `src-tauri/tests/acl_consistency.rs`

**Interfaces:**

- Consumes: `preview_path = "local-theme:<id>"` and `get_theme_preview_data_url(theme_id)` returning `string | null`.
- Produces: validated `themeApi.getPreviewDataUrl(themeId)`, a local preview component, correct local-only badges/copy, and sticky operation errors.

- [ ] **Step 1: Write failing frontend contract and hook tests**

```ts
it("accepts a local-only pack without weakening bundled rights", () => {
  const local = {
    ...snapshot.packs[0],
    id: "arina-pink",
    category: "local-import",
    preview_path: "local-theme:arina-pink",
    rights: {
      ...verifiedRights,
      commercial_redistribution: false,
      status: "local-only",
    },
  };
  expect(toThemeUiSnapshot({ ...snapshot, packs: [local] })?.packs[0]).toEqual(local);
  expect(
    toThemeUiSnapshot({ ...snapshot, packs: [{ ...local, category: "abstract" }] }),
  ).toBeNull();
});

it("keeps a failed activation message after a successful poll", async () => {
  themeApi.activate = vi.fn().mockResolvedValue({
    operation_id: "op",
    status: "failed",
    reason_codes: ["partial-apply-failed"],
    restart_required: false,
  });
  const { result } = renderHook(() => useTheme());
  await act(() => result.current.activate("observatory-muse"));
  await act(() => vi.advanceTimersByTime(5_000));
  expect(result.current.error).toMatch(/未标记为已应用/);
});
```

- [ ] **Step 2: Run the frontend tests and verify red**

Run: `npx vitest run src/lib/themeApi.test.ts src/hooks/useTheme.test.ts src/components/ThemesPage.test.tsx`

Expected: FAIL because local-only rights and preview identifiers are rejected and polling clears the error.

- [ ] **Step 3: Implement strict local preview parsing and rendering**

```ts
const BUNDLED_PREVIEW = /^\/themes\/[a-z0-9][a-z0-9./-]{0,150}$/;
const LOCAL_PREVIEW = /^local-theme:([a-z0-9]+(?:-[a-z0-9]+)*)$/;

function validRights(category: ThemeCategory, value: unknown): ThemeRights | null {
  // Bundled categories require verified + commercial redistribution.
  // local-import requires local-only + commercial_redistribution === false.
}

getPreviewDataUrl(themeId: string): Promise<string | null> {
  if (slug(themeId) === null) return Promise.reject(new Error("Invalid theme identifier"));
  return invoke("get_theme_preview_data_url", { themeId }).then(toBoundedImageDataUrl);
}
```

Render bundled `<img src={pack.preview_path}>` unchanged. For `local-import`, request the preview once, use the returned real image data URL, label the badge `仅限本机`, and show `本机素材，不参与分发` instead of `版权已核验`.

- [ ] **Step 4: Make operation errors sticky and explicit**

Change `accept(next)` so a successful snapshot updates connection state without clearing an existing operation error. Clear `error` only at the start of an explicit user operation or an explicit successful manual refresh. For every receipt outside `applied|noop`, call `setError(failureMessage(receipt))` after accepting the fresh snapshot.

- [ ] **Step 5: Add the identifier-only preview Tauri command and ACL**

```rust
#[tauri::command]
fn get_theme_preview_data_url(
    runtime: tauri::State<'_, Arc<RoutingApplication>>,
    theme_id: String,
) -> Option<String> {
    runtime.theme_preview_data_url(&theme_id)
}
```

Add only `get_theme_preview_data_url` to the invoke handler, default permission list, command permission, and ACL consistency expectations. Do not accept a path or raw bytes from the WebView.

- [ ] **Step 6: Run the frontend and ACL tests and verify green**

Run: `npx vitest run src/lib/themeApi.test.ts src/hooks/useTheme.test.ts src/components/ThemesPage.test.tsx && cargo test --manifest-path src-tauri/Cargo.toml --test acl_consistency`

Expected: PASS; bundled rights remain strict, local-only themes render their real preview, and failed operations remain visible through polling.

- [ ] **Step 7: Commit the vertical slice**

```bash
git add shared/theme-types.ts src/lib/themeApi.ts src/lib/themeApi.test.ts src/hooks/useTheme.ts src/hooks/useTheme.test.ts src/components/ThemesPage.tsx src/components/ThemesPage.test.tsx src-tauri/src/lib.rs src-tauri/permissions/default.toml src-tauri/tests/acl_consistency.rs
git commit -m "fix: surface local theme state accurately"
```

### Task 4: Install the Arina pack locally and verify the real Codex UI

**Files:**

- Create locally only: `%APPDATA%/codex-agent-monitor/routing/local-themes/arina-pink/theme.json`
- Copy locally only: `%APPDATA%/codex-agent-monitor/routing/local-themes/arina-pink/arina-pink.jpg`
- Modify: `docs/superpowers/diagnostics/2026-07-19-codex-assistant-runtime-qa.md`
- Create: `outputs/codex-arina-theme-verified.png`
- Create: `outputs/assistant-local-arina-card.png`

**Interfaces:**

- Consumes: `%LOCALAPPDATA%\CodexDreamSkin\active-theme\dream-reference.jpg`, SHA-256 `ada9d14333d0b8a08ce59d64bed1ffc33e6503ed3b141ab4dc9d1721c47af192`.
- Produces: local pack ID `arina-pink` visible in Codex Assistant and a verified themed Codex session.

- [ ] **Step 1: Create the local-only manifest with exact metadata**

```json
{
  "schema_version": 1,
  "minimum_engine_version": 1,
  "id": "arina-pink",
  "name": "Arina 粉晶花园",
  "description": "用户本机提供的柔光玫瑰主题；仅用于本机 Codex 外观。",
  "category": "local-import",
  "preview_path": "local-theme:arina-pink",
  "backdrop": {
    "kind": "image",
    "asset_id": "arina-pink",
    "overlay": "#fff5f6",
    "focal_x": 72,
    "focal_y": 45
  },
  "palette": {
    "surface": "#fff8f8",
    "surface_strong": "#fffdfd",
    "text": "#3b292d",
    "accent": "#d9637e",
    "border": "#e7aeba"
  },
  "effects": {
    "surface_opacity": 78,
    "blur_px": 10,
    "contrast_percent": 96,
    "motion": false
  },
  "assets": [
    {
      "id": "arina-pink",
      "mime_type": "image/jpeg",
      "sha256": "ada9d14333d0b8a08ce59d64bed1ffc33e6503ed3b141ab4dc9d1721c47af192"
    }
  ],
  "rights": {
    "source": "User-owned local import",
    "rightsholder": "User-provided asset",
    "license": "Local use only",
    "commercial_redistribution": false,
    "attribution": "Stored locally by user request; not redistributed",
    "reviewed_at": "2026-07-19",
    "manual_signoff": true,
    "status": "local-only"
  }
}
```

- [ ] **Step 2: Copy and independently verify the local asset**

Run:

```powershell
Copy-Item -LiteralPath "$env:LOCALAPPDATA\CodexDreamSkin\active-theme\dream-reference.jpg" -Destination "$env:APPDATA\codex-agent-monitor\routing\local-themes\arina-pink\arina-pink.jpg"
Get-FileHash -LiteralPath "$env:APPDATA\codex-agent-monitor\routing\local-themes\arina-pink\arina-pink.jpg" -Algorithm SHA256
```

Expected: `ADA9D14333D0B8A08CE59D64BED1FFC33E6503ED3B141AB4DC9D1721C47AF192`.

- [ ] **Step 3: Run the full automated quality gate**

Run: `npm run check`

Expected: TypeScript, oxlint, oxfmt, Clippy, rustfmt, Vitest, and all Rust tests PASS with no warnings promoted to errors.

- [ ] **Step 4: Build and install the local desktop package**

Run: `npm run tauri build`

Expected: a successful current-user Windows installer under `src-tauri/target/release/bundle/nsis/`; install it over the existing Codex Assistant and relaunch with WebView2 remote debugging enabled for the authorized Playwright session.

- [ ] **Step 5: Verify the Assistant catalog through Playwright Interactive**

Open the Themes page, assert one card named `Arina 粉晶花园`, assert the badge says `仅限本机`, assert the preview image is complete with non-zero natural dimensions, and capture `outputs/assistant-local-arina-card.png`.

- [ ] **Step 6: Apply Arina and verify the actual Codex DOM and pixels**

Apply `arina-pink`; if the fresh single-use restart ticket reports active native agents, use the user's already granted authorization and confirm it immediately. In every compatible Codex main page assert:

```js
const style = document.querySelector('style[data-codex-assistant-theme="arina-pink"]');
const backdrop = getComputedStyle(document.body, "::before");
({
  rules: style?.sheet?.cssRules.length ?? 0,
  image: backdrop.backgroundImage,
  mainColor: getComputedStyle(document.querySelector("main.main-surface")).color,
  sidebarColor: getComputedStyle(document.querySelector("aside.app-shell-left-panel")).color,
});
```

Expected: `rules > 0`, `image` contains `data:image/jpeg;base64,`, `image` contains neither `app://` nor `%22data`, and main/sidebar text resolves to `rgb(59, 41, 45)`. Capture `outputs/codex-arina-theme-verified.png`, inspect it visually for readable text, correct image crop, intact composer/sidebar geometry, and no second Codex window.

- [ ] **Step 7: Record the verified runtime evidence**

Append the installed executable version, local pack path, image hash, CDP assertions, screenshot paths, and Smart Routing independence check to `docs/superpowers/diagnostics/2026-07-19-codex-assistant-runtime-qa.md`.

- [ ] **Step 8: Commit repository evidence only**

```bash
git add docs/superpowers/diagnostics/2026-07-19-codex-assistant-runtime-qa.md outputs/codex-arina-theme-verified.png outputs/assistant-local-arina-card.png
git commit -m "test: verify local Arina theme end to end"
```

Do not add the local Arina manifest or image from `%APPDATA%` to Git.
