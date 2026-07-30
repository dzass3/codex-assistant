/* oxlint-disable no-await-in-loop -- Page classification must remain target-local. */
/* oxlint-disable no-underscore-dangle -- The probe inspects one namespaced theme hook. */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
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

async function themeSignal(page) {
  return page.evaluate(() => {
    const style = document.querySelector("style[data-codex-assistant-theme]");
    return {
      themeId: globalThis.__codexAssistantThemeV1?.id ?? null,
      styleCount: document.querySelectorAll("style[data-codex-assistant-theme]").length,
      styleDisabled: style?.disabled ?? null,
      pageClass: document.documentElement.getAttribute("data-codex-assistant-page-class"),
      backdrop:
        getComputedStyle(document.body, "::before").backgroundImage === "none" ? "none" : "present",
    };
  });
}

const port = requiredInteger("port");
const reapplyLocal = process.argv.includes("--reapply-local");
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
const pages = browser.contexts().flatMap((context) => context.pages());
const candidates = [];

for (const page of pages) {
  const signal = await themeSignal(page).catch(() => null);
  if (signal?.themeId && signal.styleCount === 1) candidates.push({ page, signal });
}

assert.equal(candidates.length, 1, `Expected one themed Codex page, found ${candidates.length}`);

const [{ page, signal: initialSignal }] = candidates;
let reapplied = false;

if (reapplyLocal) {
  const cargo = spawnSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--manifest-path",
      path.join(root, "src-tauri", "Cargo.toml"),
      "--example",
      "export_mock_theme_sources",
    ],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  if (cargo.status !== 0) {
    throw new Error(`theme source export failed\n${cargo.stderr}`);
  }
  const sourceExport = JSON.parse(cargo.stdout.trim());
  const theme = sourceExport.themes.find((candidate) => candidate.id === initialSignal.themeId);
  assert.ok(theme, `Current theme ${initialSignal.themeId} is not available locally`);
  reapplied = await page.evaluate(
    ({ applicationSource, verificationSource }) =>
      Boolean(globalThis.eval(applicationSource) && globalThis.eval(verificationSource)),
    {
      applicationSource: theme.application_source,
      verificationSource: theme.verification_source,
    },
  );
  assert.equal(reapplied, true, "local theme hotfix did not apply atomically");
}

const before = await themeSignal(page);
const addProjectButton = page.getByRole("button", {
  name: "添加新项目",
  exact: true,
});
let during = null;
let after = null;
let operationError = null;

try {
  await addProjectButton.hover({ force: true });
  await page.waitForTimeout(80);
  await addProjectButton.click({ noWaitAfter: true, timeout: 2_000 });
  await page.waitForTimeout(150);
  during = await themeSignal(page);
} catch (error) {
  operationError = error;
} finally {
  await page.keyboard.press("Escape").catch(() => {});
  await page.waitForTimeout(300);
  after = await themeSignal(page).catch(() => null);
}

const result = {
  verdict:
    !operationError &&
    during?.styleDisabled === false &&
    during?.pageClass === "compatible-main" &&
    during?.backdrop === "present"
      ? "green"
      : "red",
  reapplied,
  before,
  during,
  after,
};
console.log(JSON.stringify(result, null, 2));

if (operationError) throw operationError;
assert.equal(during.styleDisabled, false, "adding a project disabled the active theme");
assert.equal(
  during.pageClass,
  "compatible-main",
  "adding a project cleared the compatible page classification",
);
assert.equal(during.backdrop, "present", "adding a project removed the themed backdrop");
assert.equal(after?.styleDisabled, false, "cancelling add project did not restore the theme");
assert.equal(after?.backdrop, "present", "cancelling add project did not restore the backdrop");

// Process exit drops only this inspector transport. Never close the live Codex process.
process.exit(0);
