import assert from "node:assert/strict";
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

function colorAlpha(color) {
  if (color === "transparent") return 0;
  const slashAlpha = color.match(/\/\s*([0-9.]+)\s*\)$/);
  if (slashAlpha) return Number(slashAlpha[1]);
  const rgbaAlpha = color.match(/^rgba\([^,]+,[^,]+,[^,]+,\s*([0-9.]+)\)$/);
  if (rgbaAlpha) return Number(rgbaAlpha[1]);
  return 1;
}

const port = requiredInteger("port");
const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);

try {
  const pages = browser.contexts().flatMap((context) => context.pages());
  const pageStates = await Promise.all(
    pages.map((page) =>
      page
        .evaluate(() => {
          const surfaces = [...document.querySelectorAll("[data-plan-selection-surface]")];
          const surface = surfaces.find((candidate) => {
            const rect = candidate.getBoundingClientRect();
            return (
              rect.bottom > 0 &&
              rect.right > 0 &&
              rect.top < window.innerHeight &&
              rect.left < window.innerWidth
            );
          });
          if (!surface) return null;
          const shell = surface.closest('aside[class*="z-[41]"]');
          if (!shell) return null;
          const heading = surface.querySelector("h1,h2,h3");
          const surfaceStyle = getComputedStyle(surface);
          const shellStyle = getComputedStyle(shell);
          const headingStyle = heading ? getComputedStyle(heading) : null;
          return {
            shellBackgroundColor: shellStyle.backgroundColor,
            backgroundColor: surfaceStyle.backgroundColor,
            color: surfaceStyle.color,
            headingColor: headingStyle?.color ?? null,
            themeId: globalThis["__codexAssistantThemeV1"]?.id ?? null,
            pageClass: document.documentElement.getAttribute("data-codex-assistant-page-class"),
          };
        })
        .catch(() => null),
    ),
  );
  const candidates = pageStates.filter((state) => state !== null);

  assert.equal(
    candidates.length,
    1,
    `Expected one visible plan surface, found ${candidates.length}`,
  );
  const [state] = candidates;
  assert.ok(
    colorAlpha(state.backgroundColor) >= 0.9,
    `Plan reading surface is too transparent: ${state.backgroundColor}`,
  );
  assert.ok(
    colorAlpha(state.shellBackgroundColor) >= 0.92,
    `Expanded plan shell first frame is too transparent: ${state.shellBackgroundColor}`,
  );

  console.log(JSON.stringify({ verdict: "green", ...state }, null, 2));
  process.exit(0);
} finally {
  // Process exit drops only this inspector transport. Never close the live Codex process.
}
