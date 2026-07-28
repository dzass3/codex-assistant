/* oxlint-disable unicorn/consistent-function-scoping -- Playwright serializes these helpers. */
/* oxlint-disable no-underscore-dangle -- The harness inspects one namespaced theme hook. */
import { execFileSync } from "node:child_process";
import { chromium } from "playwright";

function requiredInteger(name) {
  const prefix = `--${name}=`;
  const argument = process.argv.find((value) => value.startsWith(prefix));
  const parsed = Number(argument?.slice(prefix.length));
  if (!Number.isInteger(parsed) || parsed <= 0) {
    throw new Error(`Missing or invalid ${prefix}<integer>`);
  }
  return parsed;
}

function argumentValue(name, fallback) {
  const prefix = `--${name}=`;
  return process.argv.find((value) => value.startsWith(prefix))?.slice(prefix.length) ?? fallback;
}

function listenerPid(port) {
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `(Get-NetTCPConnection -State Listen -LocalPort ${port} -ErrorAction Stop | Select-Object -First 1 -ExpandProperty OwningProcess)`,
    ],
    { encoding: "utf8" },
  ).trim();
  return Number(output);
}

const port = requiredInteger("port");
const expectedPid = requiredInteger("expected-pid");
const protectedPid = requiredInteger("protected-pid");
const themeId = argumentValue("theme", "seaside-blue");
const actualPid = listenerPid(port);

if (actualPid !== expectedPid) {
  throw new Error(`CDP listener identity changed: expected ${expectedPid}, got ${actualPid}`);
}
if (actualPid === protectedPid) {
  throw new Error(`Refusing to mutate protected Codex PID ${protectedPid}`);
}

const sourceJson = execFileSync(
  "cargo.exe",
  [
    "run",
    "--quiet",
    "--manifest-path",
    "src-tauri/Cargo.toml",
    "--example",
    "export_mock_theme_sources",
  ],
  { cwd: new URL("../..", import.meta.url), encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
);
const sourceExport = JSON.parse(sourceJson);
const theme = sourceExport.themes.find((candidate) => candidate.id === themeId);
if (!theme) {
  throw new Error(`Unknown theme ${themeId}`);
}

const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
try {
  const pages = browser.contexts().flatMap((context) => context.pages());
  const classifiedPages = await Promise.all(
    pages.map(async (page) => ({
      page,
      classification: await page
        .evaluate(sourceExport.classification_source)
        .catch(() => "evaluation-failed"),
    })),
  );
  const candidates = classifiedPages
    .filter(({ classification }) => classification === "compatible-main")
    .map(({ page }) => page);
  if (candidates.length !== 1) {
    throw new Error(`Expected one compatible main page, found ${candidates.length}`);
  }

  const page = candidates[0];
  await page.evaluate(sourceExport.restore_source);
  const atomicApplication =
    `(()=>{const inserted=(${theme.application_source});` +
    `return Boolean(inserted&&(${theme.verification_source}))})()`;
  const applied = await page.evaluate(atomicApplication);
  const diagnostics = await page.evaluate(() => {
    const main = document.querySelector(
      "main.main-surface,[data-testid='main-surface'],main[role='main']",
    );
    const sidebar = document.querySelector(
      "aside.app-shell-left-panel,[data-testid='app-shell-left-panel'],aside[aria-label]",
    );
    const composer = main?.querySelector(
      ".composer-surface-chrome,[data-testid='composer'],form[aria-label]",
    );
    const rect = (element) => {
      if (!element) return null;
      const value = element.getBoundingClientRect();
      return { x: value.x, y: value.y, width: value.width, height: value.height };
    };
    const hitAtCenter = (element) => {
      if (!element) return null;
      const value = element.getBoundingClientRect();
      const hit = document.elementFromPoint(value.x + value.width / 2, value.y + value.height / 2);
      return Boolean(hit && (hit === element || element.contains(hit)));
    };
    const htmlStyle = getComputedStyle(document.documentElement);
    const mainStyle = main ? getComputedStyle(main) : null;
    return {
      apiPresent: Boolean(globalThis.__codexAssistantThemeV1),
      ownedStyles: document.querySelectorAll("style[data-codex-assistant-theme]").length,
      backgroundImagePresent: htmlStyle.backgroundImage !== "none",
      backgroundImageOccurrences: (htmlStyle.backgroundImage.match(/data:image\//g) ?? []).length,
      mainRect: rect(main),
      mainBackgroundColor: mainStyle?.backgroundColor ?? null,
      sidebarRect: rect(sidebar),
      composerRect: rect(composer),
      composerHitAtCenter: hitAtCenter(composer),
    };
  });

  const result = {
    verdict: applied ? "green" : "red",
    port,
    pid: actualPid,
    protectedPid,
    themeId,
    applied,
    diagnostics,
  };
  console.log(JSON.stringify(result, null, 2));
  process.exit(applied ? 0 : 1);
} finally {
  // Process exit drops only this CDP transport. Never call browser.close() on a live Codex target.
}
