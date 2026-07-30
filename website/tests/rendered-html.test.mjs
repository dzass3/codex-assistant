import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

const siteRoot = new URL("../", import.meta.url);
const releaseVersion = "0.11.9";
const releaseAssets = [
  {
    fileName: "Codex.Assistant_0.11.9_x64-setup.exe",
    bytes: 7_054_583,
    sha256: "B4C4627E8E8157064F5E7C3F85268310A86ACBFA12ACA9774EF49071D4627AFA",
  },
  {
    fileName: "Codex.Assistant_0.11.9_x64_en-US.msi",
    bytes: 12_021_760,
    sha256: "31F2E4B86F29F91DC426248D07492C5119803E5436C3453C589C348B1FF30238",
  },
  {
    fileName: "Codex.Assistant_0.11.9_arm64-setup.exe",
    bytes: 6_755_175,
    sha256: "0CA870ADF44BEA0224183AADC83E42B740C257D55D1D9458B56578B969A950A5",
  },
  {
    fileName: "Codex.Assistant_0.11.9_arm64_en-US.msi",
    bytes: 11_870_208,
    sha256: "D011402C30341D80A66C93FF404844A3060F024F0CD506A8C80908C9F993D25A",
  },
];

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

test("server-renders the 0.11.9 theme-only showcase and four release downloads", async () => {
  const response = await render();
  assert.equal(response.status, 200);
  assert.match(response.headers.get("content-type") ?? "", /^text\/html\b/i);

  const html = await response.text();
  assert.match(html, /<title>Codex Assistant/);
  assert.match(html, /给 Codex[\s\S]*换一套皮肤/);
  assert.match(html, /一键换肤/);
  assert.match(html, /本机导入/);
  assert.match(html, /不遮文字与图标/);
  assert.match(html, /Observatory Muse/);
  assert.match(html, /12(?:<!-- -->)? 个主题/);
  assert.match(html, /不修改官方安装文件/);
  assert.match(html, /官方入口不变/);
  assert.match(html, /点击一次“应用主题”/);
  assert.match(
    html,
    new RegExp(`Codex Assistant\\s*(?:<!-- -->)?\\s*${releaseVersion.replaceAll(".", "\\.")}`),
  );
  assert.match(html, /未签名安装包/);
  assert.match(html, /SmartScreen/);
  for (const asset of releaseAssets) {
    const encoded = encodeURIComponent(asset.fileName);
    assert.match(html, new RegExp(encoded));
    assert.match(html, new RegExp(asset.sha256));
    assert.match(html, new RegExp((asset.bytes / 1_048_576).toFixed(2)));
  }
  assert.doesNotMatch(html, /0\.11\.7|0\.11\.8|Smart Routing|GPT-5\.|模型路由与观察/);
  assert.doesNotMatch(html, /\/downloads\/|codex-preview|Your site is taking shape/);
});

test("keeps public and native theme boundaries explicit", async () => {
  const [page, layout, productDemo, homepageMotion, packageJson, releaseData] = await Promise.all([
    readFile(new URL("app/page.tsx", siteRoot), "utf8"),
    readFile(new URL("app/layout.tsx", siteRoot), "utf8"),
    readFile(new URL("app/ProductDemo.tsx", siteRoot), "utf8"),
    readFile(new URL("app/HomepageMotion.tsx", siteRoot), "utf8"),
    readFile(new URL("package.json", siteRoot), "utf8"),
    readFile(new URL("app/release-data.ts", siteRoot), "utf8"),
  ]);

  assert.match(layout, /安全的一键 Codex 换肤/);
  assert.match(layout, /metadataBase/);
  assert.match(productDemo, /aria-selected/);
  assert.match(productDemo, /不会上传/);
  assert.match(productDemo, /真正的一键换肤只在 Windows 桌面版中运行/);
  assert.match(page, /releaseAssets/);
  assert.match(page, /releaseTagUrl/);
  assert.match(page, /shared\/theme-catalog\.json/);
  assert.match(page, /\/images\/observatory-hero\.webp/);
  assert.match(homepageMotion, /IntersectionObserver/);
  assert.match(homepageMotion, /prefers-reduced-motion/);
  assert.doesNotMatch(`${page}\n${productDemo}\n${layout}`, /Smart Routing|GPT-5\./);
  assert.doesNotMatch(packageJson, /react-loading-skeleton/);
  assert.equal((releaseData.match(/fileName:/g) ?? []).length, 4);
  assert.equal((releaseData.match(/sha256: "[A-F0-9]{64}"/g) ?? []).length, 4);
  await access(new URL("public/images/observatory-hero.webp", siteRoot));
  await access(new URL("public/themes/observatory-muse.jpg", siteRoot));
  await assert.rejects(access(new URL("app/_sites-preview", siteRoot)));
});
