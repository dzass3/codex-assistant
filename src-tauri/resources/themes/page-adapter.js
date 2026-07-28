() => {
  "use strict";
  if (!globalThis.document || !document.documentElement || !document.body) {
    return "invalid-target";
  }
  const present = (element) => {
    if (!element || typeof element.getBoundingClientRect !== "function") return false;
    const style = getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    if (
      style.display === "none" ||
      style.visibility === "hidden" ||
      Number(style.opacity) === 0 ||
      rect.width < 1 ||
      rect.height < 1
    ) {
      return false;
    }
    return rect.right > 0 && rect.bottom > 0 && rect.left < innerWidth && rect.top < innerHeight;
  };
  const visible = (element) => {
    if (!present(element)) return false;
    const rect = element.getBoundingClientRect();
    const x = Math.min(innerWidth - 1, Math.max(0, rect.left + rect.width / 2));
    const y = Math.min(innerHeight - 1, Math.max(0, rect.top + rect.height / 2));
    const hit = document.elementFromPoint(x, y);
    return Boolean(hit && (hit === element || element.contains(hit)));
  };
  const firstVisible = (root, selectors) => {
    for (const selector of selectors) {
      const candidate = root.querySelector(selector);
      if (visible(candidate)) return candidate;
    }
    return null;
  };
  const firstPresent = (root, selectors) => {
    for (const selector of selectors) {
      const candidate = root.querySelector(selector);
      if (present(candidate)) return candidate;
    }
    return null;
  };
  const anyVisible = (root, selectors) => Boolean(firstVisible(root, selectors));
  const sensitiveSelectors = [
    'input[type="password"]',
    'input[autocomplete="current-password"]',
    'input[autocomplete="one-time-code"]',
    "[data-auth-screen]",
    "[data-account-screen]",
    "[data-payment-screen]",
    "[data-authorization-screen]",
    "[data-permission-screen]",
    "[data-security-prompt]",
    "[data-recovery-screen]",
    'form[aria-label*="login" i]',
    'form[aria-label*="sign in" i]',
    'form[aria-label*="account" i]',
    'form[aria-label*="payment" i]',
    'form[aria-label*="permission" i]',
    'form[aria-label*="security" i]',
    'form[aria-label*="recovery" i]',
  ];
  if (anyVisible(document, sensitiveSelectors)) return "sensitive";
  const utilitySelectors = [
    '[data-codex-utility-page="true"]',
    '[data-settings-page="true"]',
    '[data-page-kind="settings"]',
    'form[aria-label*="设置"]',
    'form[aria-label*="settings" i]',
  ];
  const utility = anyVisible(document, utilitySelectors);
  const main = firstPresent(document, [
    "main.main-surface",
    'main[role="main"]',
    '[data-codex-main="true"]',
    'section[role="main"]',
  ]);
  const sidebar = firstPresent(document, [
    "aside.app-shell-left-panel",
    "aside[aria-label]",
    "nav[aria-label]",
    '[data-codex-sidebar="true"]',
  ]);
  if (!main || !sidebar) return utility ? "utility" : "unknown-build";
  const composer = firstVisible(main, [
    ".composer-surface-chrome",
    'form[aria-label*="message" i]',
    'form[data-codex-composer="true"]',
  ]);
  const homeAction = firstVisible(main, [
    '[data-codex-home-state="true"]',
    'button[aria-label*="new task" i]',
    'button[aria-label*="新任务"]',
  ]);
  if (composer) {
    const input = firstVisible(composer, [
      "textarea[aria-label]",
      "textarea",
      '[contenteditable="true"][role="textbox"]',
      "input[aria-label]",
    ]);
    const composerLabel = composer.getAttribute("aria-label");
    const knownComposer = Boolean(
      composer.matches(".composer-surface-chrome,[data-codex-composer='true']") ||
        composerLabel,
    );
    if (!input && !knownComposer) return "unknown-build";
  }
  if (composer || homeAction) return "compatible-main";
  const shellContent = firstVisible(main, [
    ".app-shell-main-content-viewport",
    "[data-page-kind]",
    "[data-settings-page]",
    "[data-codex-page]",
    "[data-testid*='page']",
    "section[aria-labelledby]",
    "table",
  ]);
  return utility || shellContent ? "compatible-shell" : "unknown-build";
}
