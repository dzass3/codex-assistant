# Security policy

## Supported version

Security fixes are provided for the latest published Codex Assistant release.

## Reporting a vulnerability

Please do not include passwords, tokens, cookies, private prompts, responses,
rollout content, local database files, or full private paths in a public issue.
Open a minimal public issue that asks the maintainer for a private reporting
channel, or use GitHub's private vulnerability reporting feature when it is
available for this repository.

Include:

- Codex Assistant version and Windows architecture;
- official ChatGPT/Codex package version;
- a minimal reproduction using non-sensitive data;
- the affected security boundary and expected behavior.

## Product boundary

Codex Assistant is a local companion for the official Microsoft Store
ChatGPT/Codex app. It does not modify the Store package, `app.asar`,
WindowsApps files, official databases, or code signatures. Local image imports
stay on the device. Theme control is restricted to an allow-listed official
process owned by the current Windows user and a random loopback endpoint.

Version 0.11.8 installers are not code-signed. Verify downloads against the
published `SHA256SUMS.txt` before installation.
