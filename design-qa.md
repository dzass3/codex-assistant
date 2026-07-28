# Codex Assistant 0.11.8 design QA

## Source of truth

- Reference screenshot: a local, non-published design reference supplied during development.
- Written acceptance criteria in the 2026-07-28 task override the reference where they intentionally differ: dark glass navigation, header, composer and output panel replace the reference's light chrome.
- The implementation must preserve the official Codex DOM, content, keyboard behavior and click targets. Theme-owned UI is limited to the empty new-task welcome and four bounded shortcuts.

## Implementation evidence

- Empty new-task state: `outputs/mock-theme-qa/10-empty-home.png`.
- Existing conversation state: `outputs/mock-theme-qa/02-applied.png`.
- Full side-by-side comparison: `outputs/design-qa-0.11.8/reference-vs-empty-home.png`.
- Focused structure comparison: `outputs/design-qa-0.11.8/focused-structure-comparison.png`.
- Final official Codex screenshot: `outputs/design-qa-0.11.8/real-codex-spring-street-final.png`.
- Reference viewport: `1194 × 762`.
- Verified implementation viewport: `1440 × 900`, device scale factor `1`.
- Official Codex viewport: `2562 × 1394`, device scale factor `1.5`.
- Verified UI state: compatible official main surface, native composer present, no visible conversation, right output panel expanded.

## Visible comparison

- **Global composition:** passed. One crisp `cover` backdrop spans the full window, including the navigation, header and output-panel tracks. The main canvas no longer has a full-page white wash.
- **Welcome hierarchy:** passed. “想构建什么？” and the four requested actions form one central empty-state composition and remain absent from the existing-conversation screenshot.
- **Material hierarchy:** passed. Navigation, header, output panel and composer use a consistent dark translucent glass family. Reading surfaces remain scoped warm-white glass cards.
- **Backdrop clarity:** passed. The image is rendered at full opacity with restrained brightness, saturation and contrast tuning; no blur or uniform opacity is applied to the artwork.
- **Foreground safety:** passed. The welcome title, shortcut labels, navigation labels, output labels and composer placeholder remain readable against their local material or focal region.
- **Responsive structure:** passed in the automated 1920×1080, 2560×1440, 3440×1440 and windowed matrix at 100%, 125%, 150% and 200% scale.
- **Intentional reference difference:** the supplied reference uses light ink-wash chrome, while the written task explicitly requires dark glass chrome. The implementation follows the written requirement without changing the reference's central welcome/card/composer hierarchy.

## Interaction and safety checks

- Shortcut cards safely focus or prefill the official composer and never send automatically.
- The welcome removes itself when conversation evidence appears.
- Theme layers are `pointer-events: none`; only the four explicit shortcut buttons accept input.
- Official buttons, menus, output-panel controls, composer controls, scrolling and content semantics remain owned by Codex.
- Login, account, payment, authorization, permission, recovery and unknown pages remain fail-closed on the official appearance.
- On the official Codex DOM, the theme remained connected after selecting 152 characters; the background stayed a fixed, non-repeating `cover` image at `74% 42%`.
- The native “运行了多个命令” disclosure completed the reversible `false → true → false` cycle without changing the theme.
- The native thread scroller completed `0 → -520 → 0`; the backdrop position and fixed attachment did not change.
- The final output-panel leaf text resolved to `rgba(255, 248, 251, 0.94)`, while assistant and tool/file reading surfaces resolved to bounded warm-white glass rather than a page-wide overlay.
- The final x64 installer completed with exit code `0`; installed ProductVersion is `0.11.8`. The protected official Codex process remained PID `39204` with start time `2026-07-27T17:26:13.1177588+08:00`.

## Release artifacts

- `outputs/release-candidates/0.11.8/Codex Assistant_0.11.8_x64-setup.exe` — SHA-256 `63959AFCC716775DF2E418798A19BF15BFE915E57B835629473CEAE87FFFC256`.
- `outputs/release-candidates/0.11.8/Codex Assistant_0.11.8_x64_en-US.msi` — SHA-256 `DED11FBF67BF8D3E228B4767165A57144C5D7D16F83D84E851DF723902086F87`.
- `outputs/release-candidates/0.11.8/Codex Assistant_0.11.8_arm64-setup.exe` — SHA-256 `E7C2D52FEBEC29E22F5B5E8E59A2F76CE0BCA407A90567777CA24C1A6B9C79ED`.
- `outputs/release-candidates/0.11.8/Codex Assistant_0.11.8_arm64_en-US.msi` — SHA-256 `1FEEAA9A2BFF6FA6E36FF12D6AAE5E905EC4F4AF5A243992423D10EE2B6C2700`.

## Remaining findings

- P0: none.
- P1: none.
- P2: none.
- P3: none.

Final result: passed
