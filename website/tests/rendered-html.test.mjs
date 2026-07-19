import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

const siteRoot = new URL("../", import.meta.url);

async function render() {
  const workerUrl = new URL("../dist/server/index.js", import.meta.url);
  workerUrl.searchParams.set("test", `${process.pid}-${Date.now()}`);
  const { default: worker } = await import(workerUrl.href);
  return worker.fetch(
    new Request("https://codex-assistant.example/", { headers: { accept: "text/html" } }),
    { ASSETS: { fetch: async () => new Response("Not found", { status: 404 }) } },
    { waitUntil() {}, passThroughOnException() {} },
  );
}

test("server-renders the complete Codex Assistant public showcase", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>Codex Assistant/);
  assert.match(html, /让每个 Codex 子代理[\s\S]*都用对模型/);
  assert.match(html, /GPT-5\.6 Luna/);
  assert.match(html, /GPT-5\.3 Codex Spark/);
  assert.match(html, /质量优先/);
  assert.match(html, /只读取本地代理元数据/);
  assert.match(html, /主题管理/);
  assert.match(html, /Observatory Muse/);
  assert.match(html, /版权已核验/);
  assert.match(html, /12 个主题/);
  assert.match(html, /票据化受控强制重启/);
  assert.match(html, /Noir Stage/);
  assert.match(html, /一套清单，桌面端与网站同步/);
  assert.match(html, /Windows 桌面版/);
  assert.doesNotMatch(html, /codex-preview|Your site is taking shape|react-loading-skeleton/);
});

test("removes starter metadata and keeps public/native capability boundaries explicit", async () => {
  const [page, layout, productDemo, homepageMotion, packageJson] = await Promise.all([
    readFile(new URL("app/page.tsx", siteRoot), "utf8"),
    readFile(new URL("app/layout.tsx", siteRoot), "utf8"),
    readFile(new URL("app/ProductDemo.tsx", siteRoot), "utf8"),
    readFile(new URL("app/HomepageMotion.tsx", siteRoot), "utf8"),
    readFile(new URL("package.json", siteRoot), "utf8"),
  ]);

  assert.doesNotMatch(page, /codex-preview|_sites-preview|SkeletonPreview/);
  assert.match(layout, /Codex Assistant/);
  assert.match(layout, /metadataBase/);
  assert.match(productDemo, /aria-selected/);
  assert.match(productDemo, /仅展示脱敏示例数据/);
  assert.match(productDemo, /控制本机 Codex 需要 Windows 桌面版/);
  assert.match(page, /\/downloads\/Codex-Assistant-0\.6\.0-x64-setup\.exe/);
  assert.match(page, /3,747,870 bytes/);
  assert.match(page, /d88382969a45/);
  assert.match(page, /shared\/theme-catalog\.json/);
  assert.match(page, /\/images\/observatory-hero\.webp/);
  assert.match(homepageMotion, /IntersectionObserver/);
  assert.match(homepageMotion, /prefers-reduced-motion/);
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);
  await access(new URL("public/downloads/Codex-Assistant-0.6.0-x64-setup.exe", siteRoot));
  await access(new URL("public/downloads/Codex-Assistant-0.5.0-x64-setup.exe", siteRoot));
  await access(new URL("public/images/observatory-hero.webp", siteRoot));
  await assert.rejects(access(new URL("app/_sites-preview", siteRoot)));
});
