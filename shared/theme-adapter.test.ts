import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { chromium, type Browser, type Page } from "playwright";

const adapterPath = resolve("src-tauri/resources/themes/page-adapter.js");
const adapterSource = readFileSync(adapterPath, "utf8");

let browser: Browser;
let page: Page;

beforeAll(async () => {
  browser = await chromium.launch({ headless: true });
  page = await browser.newPage({ viewport: { width: 1100, height: 760 } });
});

afterAll(async () => {
  await browser.close();
});

async function classify(markup: string): Promise<string> {
  await page.setContent(
    `<style>*{box-sizing:border-box}body{margin:0}main,aside,nav,form,[role=main]{min-width:120px;min-height:80px;display:block}</style>${markup}`,
  );
  return page.evaluate((source) => {
    const classifier = (0, eval)(`(${source})`) as () => string;
    return classifier();
  }, adapterSource);
}

describe("shared version-adaptive page adapter", () => {
  it("accepts current and alternate main-page structures using three independent capabilities", async () => {
    await expect(
      classify(
        '<aside class="app-shell-left-panel"></aside><main class="main-surface"><form class="composer-surface-chrome"><textarea aria-label="Message"></textarea></form></main>',
      ),
    ).resolves.toBe("compatible-main");
    await expect(
      classify(
        '<nav aria-label="Projects"></nav><section role="main"><form aria-label="Send a message"><textarea aria-label="Message"></textarea><button aria-label="Send"></button></form></section>',
      ),
    ).resolves.toBe("compatible-main");
    await expect(
      classify(
        '<aside aria-label="Projects"></aside><main role="main"><button aria-label="New task"></button></main>',
      ),
    ).resolves.toBe("compatible-main");
    await expect(
      classify(
        '<div data-codex-sidebar="true" style="width:180px;height:200px"></div><div data-codex-main="true" style="width:700px;height:400px"><form data-codex-composer="true"><div contenteditable="true" role="textbox" style="width:300px;height:60px"></div></form></div>',
      ),
    ).resolves.toBe("compatible-main");
  }, 15_000);

  it("does not accept one brittle selector as sufficient evidence", async () => {
    await expect(classify('<main class="main-surface"></main>')).resolves.toBe("unknown-build");
  });

  it("accepts every visible non-sensitive official shell, including sites and settings", async () => {
    await expect(
      classify(
        '<aside class="app-shell-left-panel"></aside><main class="main-surface"><section data-page-kind="sites" style="width:700px;height:400px"><input aria-label="搜索站点"><button aria-label="创建"></button></section></main>',
      ),
    ).resolves.toBe("compatible-shell");
    await expect(
      classify(
        '<aside class="app-shell-left-panel"></aside><main class="main-surface"><section data-settings-page="true" style="width:700px;height:400px"></section></main>',
      ),
    ).resolves.toBe("compatible-shell");
    await expect(
      classify(
        '<nav aria-label="Projects"></nav><section role="main"><section data-page-kind="plugins" style="width:700px;height:400px"></section></section>',
      ),
    ).resolves.toBe("compatible-shell");
    await expect(
      classify(
        '<aside class="app-shell-left-panel"></aside><main class="main-surface"><header class="app-header-tint"></header><div class="app-shell-main-content-viewport" style="width:700px;height:400px"><input placeholder="search"></div></main>',
      ),
    ).resolves.toBe("compatible-shell");
  });

  it("keeps sensitive, invisible, detached utility and unknown pages official", async () => {
    await expect(
      classify(
        '<aside class="app-shell-left-panel"></aside><main class="main-surface"><form class="composer-surface-chrome"></form><input type="password">',
      ),
    ).resolves.toBe("sensitive");
    await expect(
      classify('<section data-settings-page="true" style="width:300px;height:200px"></section>'),
    ).resolves.toBe("utility");
    await expect(
      classify(
        '<aside class="app-shell-left-panel" style="display:none"></aside><main class="main-surface"><form class="composer-surface-chrome"></form></main>',
      ),
    ).resolves.toBe("unknown-build");
    await expect(classify("<main></main>")).resolves.toBe("unknown-build");
  });

  it("uses bounded structural metadata and never reads conversation content", () => {
    for (const forbidden of [
      "textContent",
      "innerText",
      "innerHTML",
      "location.href",
      "localStorage",
      "sessionStorage",
    ]) {
      expect(adapterSource).not.toContain(forbidden);
    }
    expect(adapterSource).toContain("getAttribute");
    expect(adapterSource).toContain("elementFromPoint");
  });
});
