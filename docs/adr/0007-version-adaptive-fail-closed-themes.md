# ADR 0007: Version-adaptive, fail-closed theme compatibility

- Status: Accepted
- Date: 2026-07-27

Codex Assistant previously treated a small set of fixed CSS selectors as proof that a Codex page was compatible. Official application updates can preserve those selectors while changing page meaning, or replace them while leaving the page safe to theme, so selector-only matching can both reject supported pages and apply unsafe styles.

Theme compatibility is therefore expressed through release-versioned adapters that combine official package identity, page structure, ARIA semantics, visibility, hit testing and expected main-page capabilities. Rich theming is limited to one verified primary home, project or task page; sensitive and unknown pages remain official, and utility, preload, DevTools and invisible targets do not turn an otherwise successful primary-page operation into a failure. An unknown official build fails closed and is never reported as applied.

This trades universal best-effort injection for explicit compatibility evidence, regression fixtures and honest unsupported guidance. New official builds require an adapter or compatible fingerprint before support is claimed.
