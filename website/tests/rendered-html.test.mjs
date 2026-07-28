import assert from "node:assert/strict";
import { access, readFile } from "node:fs/promises";
import test from "node:test";

const siteRoot = new URL("../", import.meta.url);
const releaseVersion = "0.11.8";
const releaseAssets = [
  {
    fileName: "Codex.Assistant_0.11.8_x64-setup.exe",
    bytes: 7_710_295,
    sha256: "63959AFCC716775DF2E418798A19BF15BFE915E57B835629473CEAE87FFFC256",
  },
  {
    fileName: "Codex.Assistant_0.11.8_x64_en-US.msi",
    bytes: 12_460_032,
    sha256: "DED11FBF67BF8D3E228B4767165A57144C5D7D16F83D84E851DF723902086F87",
  },
  {
    fileName: "Codex.Assistant_0.11.8_arm64-setup.exe",
    bytes: 7_426_966,
    sha256: "E7C2D52FEBEC29E22F5B5E8E59A2F76CE0BCA407A90567777CA24C1A6B9C79ED",
  },
  {
    fileName: "Codex.Assistant_0.11.8_arm64_en-US.msi",
    bytes: 12_308_480,
    sha256: "1FEEAA9A2BFF6FA6E36FF12D6AAE5E905EC4F4AF5A243992423D10EE2B6C2700",
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

test("server-renders the 0.11.8 theme-only showcase and four release downloads", async () => {
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
  assert.match(html, /14(?:<!-- -->)? 个主题/);
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
  assert.doesNotMatch(html, /0\.11\.7|Smart Routing|GPT-5\.|模型路由与观察/);
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
