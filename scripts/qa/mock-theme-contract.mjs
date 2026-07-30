/* oxlint-disable no-await-in-loop -- Themes must be applied sequentially in one DOM. */
/* oxlint-disable no-underscore-dangle -- The harness inspects namespaced window hooks. */
/* oxlint-disable unicorn/consistent-function-scoping -- Playwright serializes this callback. */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const outputDirectory = path.join(root, "outputs", "mock-theme-qa");
const canary = process.env.MOCK_THEME_CANARY ?? "";
const captureGallery = process.env.MOCK_THEME_CAPTURE_GALLERY === "1";

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
assert.ok(sourceExport.themes.length >= 2, "at least two bundled themes are required");
assert.ok(sourceExport.local_theme.id.startsWith("local-"), "real local import is required");

const replacementLandscapeThemeIds = new Set([
  "seaside-blue",
  "autumn-wuxia",
  "meteor-evening",
  "fuji-autumn",
]);

assert.deepEqual(
  sourceExport.themes
    .filter((theme) => replacementLandscapeThemeIds.has(theme.id))
    .map((theme) => theme.id)
    .toSorted(),
  [...replacementLandscapeThemeIds].toSorted(),
  "the replacement landscape contract must cover all four stable theme ids",
);

function assertBackdropLayout(theme, backdrop) {
  assert.equal(backdrop.backgroundSize, "cover", `${theme.id} lost cover sizing`);
  assert.equal(backdrop.backgroundRepeat, "no-repeat", `${theme.id} started tiling`);
  assert.notEqual(
    backdrop.backgroundPosition,
    "50% 50%",
    `${theme.id} lost the reviewed subject focal point`,
  );
}

await fs.mkdir(outputDirectory, { recursive: true });

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });

const mockHtml = `<!doctype html>
<html lang="zh-CN" data-codex-window-type="electron">
  <head>
    <meta charset="utf-8" />
    <style>
      :root {
        --color-token-main-surface-primary: #f7f7f8;
        --color-token-sidebar-surface-primary: #efeff1;
        --color-token-foreground: rgba(26, 28, 31, 0.85);
        --color-token-description-foreground: rgba(26, 28, 31, 0.49);
        --color-token-conversation-body: rgba(26, 28, 31, 0.60);
        --color-token-conversation-summary-leading: rgba(26, 28, 31, 0.49);
        --color-token-conversation-summary-trailing: rgba(26, 28, 31, 0.42);
        --color-token-text-secondary: rgba(26, 28, 31, 0.65);
        --color-token-text-tertiary: rgba(26, 28, 31, 0.49);
        --color-token-text-primary: rgb(26, 28, 31);
        --color-token-editor-foreground: rgb(26, 28, 31);
        --color-token-icon-foreground: rgb(26, 28, 31);
        --color-token-application-menu-foreground: rgb(26, 28, 31);
        --color-token-dropdown-foreground: rgb(26, 28, 31);
        --color-token-input-foreground: rgb(26, 28, 31);
        --color-text-button-secondary: rgb(26, 28, 31);
        --vscode-foreground: rgb(26, 28, 31);
        --vscode-sideBar-foreground: rgb(26, 28, 31);
        --vscode-menu-foreground: rgb(26, 28, 31);
        --vscode-input-foreground: rgb(26, 28, 31);
      }
      * { box-sizing: border-box; }
      html, body, #root { width: 100%; min-height: 100%; margin: 0; }
      html { background: #f7f7f8; }
      [data-codex-window-type="electron"] { background: 0 0; }
      body { color: #202123; font: 16px/1.5 system-ui, sans-serif; }
      button, textarea, select { font: inherit; }
      .official-token-foreground { color: var(--color-token-foreground); }
      .official-token-muted { color: var(--color-token-description-foreground); }
      ._ModelPickerTriggerModelLabel_mock { color: rgb(26, 28, 31); }
      #status-running-label { color: var(--color-token-conversation-body) !important; }
      .loading-shimmer-pure-text { color: rgba(26, 28, 31, 0.385); }
      #root { display: flex; min-height: 900px; }
      aside.app-shell-left-panel {
        width: 280px; flex: 0 0 280px; padding: 22px 18px; background: #efeff1;
      }
      aside nav { display: grid; gap: 10px; }
      aside button {
        width: 100%; border: 0; border-radius: 12px; padding: 12px 14px;
        color: #202123; background: transparent; text-align: left; cursor: pointer;
      }
      aside button[aria-current="page"] { background: #dedee3; }
      main.main-surface { flex: 1; min-width: 0; background: #f7f7f8; }
      [data-codex-output-panel] {
        width: 300px; flex: 0 0 300px; margin: 72px 18px 112px 0; padding: 18px;
        overflow: auto; border: 1px solid #dedee3; border-radius: 14px; background: #fff;
      }
      [data-codex-output-panel] button {
        width: 100%; min-height: 38px; margin-top: 8px; border: 0; border-radius: 10px;
        background: transparent; text-align: left;
      }
      .app-header-tint {
        height: 68px; display: flex; align-items: center; gap: 12px;
        padding: 0 32px; background: #f7f7f8; border-bottom: 1px solid #dedee3;
      }
      #native-titlebar {
        position: fixed; top: 0; right: 0; z-index: 100; width: 240px;
      }
      .content { max-width: 920px; margin: 0 auto; padding: 42px 34px 180px; }
      .semantic-text { color: rgb(32, 33, 35); font-size: 27px; font-weight: 700; }
      .assistant-copy { color: rgb(55, 56, 60); }
      .content-semantic-icon { width: 24px; height: 24px; color: rgb(32, 33, 35); }
      .content-image {
        display: block; width: 320px; height: 180px; border-radius: 18px;
        object-fit: cover; border: 1px solid #c9c9ce;
      }
      .icon-button, .primary-action, .send-button {
        border: 0; cursor: pointer; display: inline-flex; align-items: center;
        justify-content: center; gap: 8px;
      }
      .icon-button { width: 42px; height: 42px; color: rgb(32, 33, 35); background: transparent; }
      .icon-button svg { width: 24px; height: 24px; }
      .primary-action {
        margin-top: 20px; padding: 12px 18px; border-radius: 999px;
        color: rgb(255, 255, 255); background: rgb(16, 163, 127);
      }
      [data-user-message-bubble="true"] { margin-top: 28px; padding: 18px; background: #e9e9ed; }
      .bg-token-dropdown-background { margin-top: 20px; padding: 14px; background: #fff; border: 1px solid #ddd; }
      #sites-sticky-fade::after {
        content: "";
        position: absolute;
        top: 100%;
        right: 0;
        left: 0;
        height: 32px;
        pointer-events: none;
        background-image: linear-gradient(rgb(255, 255, 255), rgba(0, 0, 0, 0));
      }
      #composer-bottom-fade {
        position: absolute;
        right: 0;
        bottom: 100%;
        left: 0;
        height: 28px;
        pointer-events: none;
        background-image: linear-gradient(to top, rgb(255, 255, 255), rgba(0, 0, 0, 0));
      }
      .composer-surface-chrome {
        position: fixed; z-index: 20; left: 340px; right: 340px; bottom: 28px;
        display: flex; align-items: end; gap: 12px; padding: 16px;
        border: 1px solid #d3d3d8; border-radius: 22px; background: #fff;
      }
      .composer-surface-chrome textarea {
        flex: 1; min-height: 58px; resize: none; border: 0; outline: 0;
        color: rgb(32, 33, 35); background: transparent;
      }
      .send-button {
        width: 44px; height: 44px; border-radius: 50%; color: #fff; background: #111;
      }
      .interaction-lab { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 20px; }
      .interaction-lab > button, .interaction-lab > a, .interaction-lab > select {
        min-height: 38px; padding: 8px 12px; border: 1px solid #c9c9ce;
        border-radius: 10px; color: #202123; background: #fff;
      }
      #mock-menu { padding: 10px; border: 1px solid #ccc; background: #fff; }
      #mock-dialog { border: 1px solid #bbb; border-radius: 14px; }
      #scroll-box { width: 100%; height: 90px; margin-top: 12px; overflow: auto; border: 1px solid #ccc; }
      #scroll-box > div { height: 320px; padding: 12px; }
      @media (max-width: 1200px) {
        aside.app-shell-left-panel { width: 220px; flex-basis: 220px; }
        [data-codex-output-panel] { width: 240px; flex-basis: 240px; }
        .composer-surface-chrome { left: 250px; right: 270px; }
      }
    </style>
  </head>
  <body>
    <div id="native-titlebar" class="app-header-tint group/application-menu-top-bar">
      <button id="window-control" type="button" aria-label="窗口控制">□</button>
    </div>
    <div id="root">
      <aside class="app-shell-left-panel">
        <h1>Codex</h1>
        <nav>
          <button id="new-task" type="button">
            <span id="new-task-label" class="official-token-foreground">新建任务</span>
          </button>
          <button id="active-thread" type="button" aria-current="page">Mock 主题安全测试</button>
          <button id="settings" type="button">设置</button>
          <div id="project-row" role="button" tabindex="0">
            <span id="project-row-label" class="official-token-foreground">真实项目行</span>
          </div>
        </nav>
      </aside>
      <main class="main-surface">
        <header class="app-header-tint">
          <button id="icon-button" class="icon-button" type="button" aria-label="工具">
            <svg viewBox="0 0 24 24" aria-hidden="true"><path id="semantic-icon" fill="currentColor" d="M4 5h16v3H4zm0 6h16v3H4zm0 6h16v3H4z" /></svg>
          </button>
          <strong>Mock ChatGPT / Codex</strong>
          <button
            id="model-selector"
            type="button"
            aria-haspopup="listbox"
            aria-expanded="false"
          >
            <span id="header-model-label" class="official-token-muted">5.6 Sol 极高</span>
          </button>
        </header>
        <section class="content" data-codex-conversation="true">
          <h2 id="semantic-text" class="semantic-text">主题不能覆盖这段 ChatGPT 文字</h2>
          <p id="assistant-copy" class="assistant-copy">正文、图标、图片和交互必须保持官方语义与功能。</p>
          <svg id="content-semantic-icon" class="content-semantic-icon" viewBox="0 0 24 24" aria-label="正文语义图标"><path fill="currentColor" d="M12 2 22 12 12 22 2 12Z" /></svg>
          <img id="content-image" class="content-image" alt="测试内容图片" src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='640' height='360'%3E%3Cdefs%3E%3ClinearGradient id='g' x2='1' y2='1'%3E%3Cstop stop-color='%2364e7ff'/%3E%3Cstop offset='1' stop-color='%23a990ff'/%3E%3C/linearGradient%3E%3C/defs%3E%3Crect width='640' height='360' rx='32' fill='url(%23g)'/%3E%3Ccircle cx='500' cy='100' r='60' fill='%23fff' fill-opacity='.55'/%3E%3C/svg%3E" />
          <button id="primary-action" class="primary-action" type="button">正常功能按钮</button>
          <div data-user-message-bubble="true">用户消息气泡仍需清晰可读</div>
          <div data-content-search-unit-key="mock:assistant">
            <div data-response-annotation-target="mock-assistant-message">
              助手长文本只使用局部阅读卡片。
            </div>
          </div>
          <span
            id="streaming-preparation"
            class="loading-shimmer-pure-text min-w-0 flex-1 truncate select-none"
            aria-hidden="false"
          >
            Preparing imagegen and frontend-design skills
          </span>
          <div id="status-running" data-testid="status-running" data-status="running" aria-busy="true">
            <strong id="status-running-label">正在分析</strong><span>运行中 · 1.2 秒</span>
          </div>
          <div id="status-success" data-local-conversation-item-target-ids="call_mock" data-status="success">
            <strong>文件检查</strong><span>成功 · 0.8 秒</span>
          </div>
          <div id="status-warning" data-testid="status-warning" data-status="warning">
            <strong>兼容性提示</strong><span>警告 · 需要注意</span>
          </div>
          <div id="status-error" data-testid="status-error" data-status="error">
            <strong>构建任务</strong><span>失败 · 查看详情</span>
          </div>
          <div id="status-info" data-testid="status-info" data-status="info">
            <strong>普通信息</strong><span>已记录</span>
          </div>
          <div class="bg-token-dropdown-background">下拉面板内容不能被遮住</div>
          <div class="interaction-lab">
            <button id="menu-trigger" type="button" aria-haspopup="menu" aria-expanded="false">打开菜单</button>
            <div id="mock-menu" role="menu" hidden>
              <button id="menu-item" role="menuitem" type="button">
                <span id="menu-item-label" class="official-token-foreground">菜单项</span>
              </button>
            </div>
            <button id="dialog-trigger" type="button">打开对话框</button>
            <select id="native-dropdown" aria-label="测试下拉选项"><option value="one">选项一</option><option value="two">选项二</option></select>
            <a id="safe-link" href="#linked-target">测试链接</a>
          </div>
          <div id="scroll-box" tabindex="0"><div>可滚动内容<div id="linked-target">链接目标</div></div></div>
          <dialog id="mock-dialog"><p>对话框保持可用</p><button id="dialog-close" type="button">关闭对话框</button></dialog>
        </section>
        <section id="home-state" data-codex-home-state="true" hidden></section>
        <form
          id="composer"
          class="composer-surface-chrome bg-token-input-background/90 electron:dark:bg-token-dropdown-background"
        >
          <div
            id="composer-bottom-fade"
            class="pointer-events-none absolute inset-x-0 -bottom-1 h-7 bg-gradient-to-t from-token-main-surface-primary to-transparent"
          ></div>
          <button
            id="composer-attachment"
            data-composer-attachment-pill="true"
            class="composer-attachment-surface text-token-foreground bg-token-dropdown-background"
            type="button"
          >
            <span id="composer-attachment-label">Recreate this exact design</span>
          </button>
          <textarea id="composer-input" aria-label="消息输入" placeholder="输入一条测试消息"></textarea>
          <button id="composer-model-selector" type="button" aria-haspopup="listbox">
            <span
              id="composer-model-label"
              class="official-token-foreground _ModelPickerTriggerModelLabel_mock"
            >5.6 Sol</span>
            <span id="composer-effort-label" class="official-token-muted">极高</span>
          </button>
          <button id="send-button" class="send-button" type="submit" aria-label="发送">↑</button>
        </form>
      </main>
      <aside id="output-panel" data-codex-output-panel="true" aria-label="输出">
        <header class="bg-token-dropdown-background">输出</header>
        <section role="group" aria-label="计划">
          <strong>计划</strong>
          <button id="output-task" type="button">子任务</button>
        </section>
        <section role="group" aria-label="来源">
          <strong>来源</strong>
          <button id="output-source" type="button">
            <span id="output-source-label" class="official-token-foreground">Mock 源文件</span>
          </button>
        </section>
      </aside>
    </div>
    <script>
      window.__mockEvents = { primary: 0, sidebar: 0, icon: 0, output: 0, menu: 0, dialog: 0, link: 0, sent: [] };
      document.querySelector('#primary-action').addEventListener('click', () => window.__mockEvents.primary++);
      document.querySelector('#active-thread').addEventListener('click', () => window.__mockEvents.sidebar++);
      document.querySelector('#icon-button').addEventListener('click', () => window.__mockEvents.icon++);
      document.querySelector('#output-task').addEventListener('click', () => window.__mockEvents.output++);
      document.querySelector('#menu-trigger').addEventListener('click', () => {
        document.querySelector('#mock-menu').hidden = false;
        document.querySelector('#menu-trigger').setAttribute('aria-expanded', 'true');
      });
      document.querySelector('#menu-item').addEventListener('click', () => {
        window.__mockEvents.menu++;
        document.querySelector('#mock-menu').hidden = true;
        document.querySelector('#menu-trigger').setAttribute('aria-expanded', 'false');
      });
      document.querySelector('#dialog-trigger').addEventListener('click', () => {
        window.__mockEvents.dialog++;
        document.querySelector('#mock-dialog').showModal();
      });
      document.querySelector('#dialog-close').addEventListener('click', () => document.querySelector('#mock-dialog').close());
      document.querySelector('#safe-link').addEventListener('click', (event) => {
        event.preventDefault();
        window.__mockEvents.link++;
      });
      document.querySelector('#composer').addEventListener('submit', (event) => {
        event.preventDefault();
        window.__mockEvents.sent.push(document.querySelector('#composer-input').value);
      });
    </script>
  </body>
</html>`;

function snapshotExpression() {
  return () => {
    const inspect = (selector) => {
      const element = document.querySelector(selector);
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      const x = Math.min(innerWidth - 1, Math.max(0, rect.left + rect.width / 2));
      const y = Math.min(innerHeight - 1, Math.max(0, rect.top + rect.height / 2));
      const hit = document.elementFromPoint(x, y);
      return {
        selector,
        text: element.textContent,
        color: style.color,
        backgroundColor: style.backgroundColor,
        display: style.display,
        visibility: style.visibility,
        opacity: style.opacity,
        pointerEvents: style.pointerEvents,
        width: rect.width,
        height: rect.height,
        hitId: hit?.id ?? null,
        hitWithinTarget: Boolean(hit && (hit === element || element.contains(hit))),
      };
    };

    const image = document.querySelector("#content-image");
    const backdrop = getComputedStyle(document.body, "::before");
    const scrim = getComputedStyle(document.body, "::after");
    const sidebar = getComputedStyle(document.querySelector("aside.app-shell-left-panel"));
    const header = getComputedStyle(document.querySelector("main .app-header-tint"));
    const titlebar = getComputedStyle(document.querySelector("#native-titlebar"));
    const outputPanel = getComputedStyle(document.querySelector("[data-codex-output-panel]"));
    const outputHeader = getComputedStyle(
      document.querySelector("[data-codex-output-panel] header"),
    );
    const composer = getComputedStyle(document.querySelector(".composer-surface-chrome"));
    const userBubble = getComputedStyle(document.querySelector("[data-user-message-bubble]"));
    const assistantReading = getComputedStyle(
      document.querySelector(
        '[data-content-search-unit-key$=":assistant"]>[data-response-annotation-target]',
      ),
    );
    const toolReading = getComputedStyle(document.querySelector("#status-success"));
    const composerFade = getComputedStyle(document.querySelector("#composer-bottom-fade"));
    const rootStyle = getComputedStyle(document.documentElement);
    const statusStyles = Object.fromEntries(
      ["running", "success", "warning", "error", "info"].map((status) => {
        const style = getComputedStyle(document.querySelector(`#status-${status}`));
        return [
          status,
          {
            backgroundColor: style.backgroundColor,
            borderLeftColor: style.borderLeftColor,
            borderLeftWidth: style.borderLeftWidth,
            color: style.color,
            height: document.querySelector(`#status-${status}`).getBoundingClientRect().height,
            marginTop: style.marginTop,
            paddingTop: style.paddingTop,
            paddingBottom: style.paddingBottom,
          },
        ];
      }),
    );
    return {
      adaptiveTokens: {
        luminance: rootStyle.getPropertyValue("--codex-assistant-background-luminance").trim(),
        complexity: rootStyle.getPropertyValue("--codex-assistant-background-complexity").trim(),
        saturation: rootStyle.getPropertyValue("--codex-assistant-background-saturation").trim(),
        surface1: rootStyle.getPropertyValue("--surface-1").trim(),
        surface2: rootStyle.getPropertyValue("--surface-2").trim(),
        surface3: rootStyle.getPropertyValue("--surface-3").trim(),
        overlayStrong: rootStyle.getPropertyValue("--bg-overlay-strong").trim(),
      },
      htmlBackground: getComputedStyle(document.documentElement).backgroundImage,
      htmlBackgroundColor: getComputedStyle(document.documentElement).backgroundColor,
      backdrop: {
        backgroundImage: backdrop.backgroundImage,
        backgroundSize: backdrop.backgroundSize,
        backgroundRepeat: backdrop.backgroundRepeat,
        backgroundPosition: backdrop.backgroundPosition,
        backgroundAttachment: backdrop.backgroundAttachment,
        pointerEvents: backdrop.pointerEvents,
        filter: backdrop.filter,
      },
      scrim: {
        backgroundImage: scrim.backgroundImage,
        pointerEvents: scrim.pointerEvents,
      },
      mainBackground: getComputedStyle(document.querySelector("main.main-surface")).backgroundColor,
      mainBackgroundImage: getComputedStyle(document.querySelector("main.main-surface"))
        .backgroundImage,
      sidebarMaterial: {
        backgroundColor: sidebar.backgroundColor,
        backdropFilter: sidebar.backdropFilter,
        borderRightColor: sidebar.borderRightColor,
        boxShadow: sidebar.boxShadow,
        color: sidebar.color,
      },
      headerMaterial: {
        backgroundColor: header.backgroundColor,
        backdropFilter: header.backdropFilter,
        borderBottomColor: header.borderBottomColor,
      },
      titlebarMaterial: {
        backgroundColor: titlebar.backgroundColor,
        backdropFilter: titlebar.backdropFilter,
        color: titlebar.color,
      },
      windowControl: inspect("#window-control"),
      outputMaterial: {
        backgroundColor: outputPanel.backgroundColor,
        backdropFilter: outputPanel.backdropFilter,
        borderColor: outputPanel.borderColor,
        borderRadius: outputPanel.borderRadius,
        boxShadow: outputPanel.boxShadow,
      },
      outputHeaderBackground: outputHeader.backgroundColor,
      composerMaterial: {
        backgroundColor: composer.backgroundColor,
        backdropFilter: composer.backdropFilter,
        borderColor: composer.borderColor,
        borderRadius: composer.borderRadius,
        boxShadow: composer.boxShadow,
        color: composer.color,
      },
      composerFade: {
        backgroundImage: composerFade.backgroundImage,
      },
      composerAttachment: inspect("#composer-attachment"),
      composerAttachmentLabel: inspect("#composer-attachment-label"),
      userBubbleMaterial: {
        backgroundColor: userBubble.backgroundColor,
        borderColor: userBubble.borderColor,
        borderRadius: userBubble.borderRadius,
        boxShadow: userBubble.boxShadow,
      },
      assistantReadingBackground: assistantReading.backgroundColor,
      streamingPreparation: inspect("#streaming-preparation"),
      toolReadingBackground: toolReading.backgroundColor,
      statusStyles,
      semanticText: inspect("#semantic-text"),
      assistantCopy: inspect("#assistant-copy"),
      image: {
        ...inspect("#content-image"),
        src: image.currentSrc,
        naturalWidth: image.naturalWidth,
        naturalHeight: image.naturalHeight,
      },
      iconButton: inspect("#icon-button"),
      modelSelector: inspect("#model-selector"),
      chromeDescendants: {
        sidebarPrimary: inspect("#new-task-label"),
        sidebarRoleButton: inspect("#project-row-label"),
        headerMuted: inspect("#header-model-label"),
        composerPrimary: inspect("#composer-model-label"),
        composerMuted: inspect("#composer-effort-label"),
        menuPrimary: inspect("#menu-item-label"),
        outputPrimary: inspect("#output-source-label"),
        statusPrimary: inspect("#status-running-label"),
      },
      iconFill: getComputedStyle(document.querySelector("#content-semantic-icon path")).fill,
      headerIconFill: getComputedStyle(document.querySelector("#semantic-icon")).fill,
      primaryAction: inspect("#primary-action"),
      outputTask: inspect("#output-task"),
      composer: inspect("#composer"),
      input: inspect("#composer-input"),
      styleCount: document.querySelectorAll("style[data-codex-assistant-theme]").length,
      themeId: globalThis.__codexAssistantThemeV1?.id ?? null,
      styleDisabled: document.querySelector("style[data-codex-assistant-theme]")?.disabled ?? null,
      pageClass: document.documentElement.getAttribute("data-codex-assistant-page-class"),
      welcomeVisible: document.querySelector("[data-codex-assistant-theme-welcome]") !== null,
      welcomeCardCount: document.querySelectorAll("[data-codex-assistant-welcome-action]").length,
      bodyBefore: getComputedStyle(document.body, "::before").content,
      bodyAfter: getComputedStyle(document.body, "::after").content,
      costlyGlassCount: [...document.querySelectorAll("*")].filter((element) => {
        const rect = element.getBoundingClientRect();
        const style = getComputedStyle(element);
        return (
          rect.width > 1 &&
          rect.height > 1 &&
          rect.bottom > 0 &&
          rect.right > 0 &&
          rect.top < innerHeight &&
          rect.left < innerWidth &&
          style.backdropFilter !== "none"
        );
      }).length,
      costlyGlassPixelRatio:
        [...document.querySelectorAll("*")].reduce((total, element) => {
          const rect = element.getBoundingClientRect();
          const style = getComputedStyle(element);
          if (style.backdropFilter === "none") return total;
          const visibleWidth = Math.max(
            0,
            Math.min(innerWidth, rect.right) - Math.max(0, rect.left),
          );
          const visibleHeight = Math.max(
            0,
            Math.min(innerHeight, rect.bottom) - Math.max(0, rect.top),
          );
          return total + visibleWidth * visibleHeight;
        }, 0) /
        (innerWidth * innerHeight),
    };
  };
}

function pageClassExpression() {
  return (source) => ({
    classification: globalThis.eval(source),
    styleCount: document.querySelectorAll("style[data-codex-assistant-theme]").length,
    styleDisabled: document.querySelector("style[data-codex-assistant-theme]")?.disabled ?? null,
    pageClass: document.documentElement.getAttribute("data-codex-assistant-page-class"),
    backdrop: getComputedStyle(document.body, "::before").backgroundImage,
    backdropPosition: getComputedStyle(document.body, "::before").backgroundPosition,
    mainBackground: getComputedStyle(document.querySelector("main.main-surface")).backgroundColor,
    mainBackgroundImage: getComputedStyle(document.querySelector("main.main-surface"))
      .backgroundImage,
  });
}

function assertSemanticContentPreserved(before, after, requireAllHits = true) {
  for (const key of ["semanticText", "assistantCopy"]) {
    assert.equal(after[key].text, before[key].text, `${key} text changed`);
    assert.equal(after[key].color, before[key].color, `${key} color changed`);
    assert.equal(after[key].visibility, "visible", `${key} is hidden`);
    assert.equal(after[key].opacity, "1", `${key} became transparent`);
    if (requireAllHits) assert.ok(after[key].hitWithinTarget, `${key} is covered by another layer`);
  }
  assert.equal(after.image.src, before.image.src, "content image source changed");
  assert.equal(after.image.naturalWidth, before.image.naturalWidth, "content image width changed");
  assert.equal(
    after.image.naturalHeight,
    before.image.naturalHeight,
    "content image height changed",
  );
  if (requireAllHits) {
    assert.ok(after.image.hitWithinTarget, "content image is covered by another layer");
  }
  assert.equal(after.iconFill, before.iconFill, "semantic SVG fill changed");
  assert.equal(
    after.primaryAction.color,
    before.primaryAction.color,
    "primary action text changed",
  );
  assert.equal(
    after.primaryAction.backgroundColor,
    before.primaryAction.backgroundColor,
    "primary action fill changed",
  );
  for (const key of [
    "iconButton",
    "modelSelector",
    "primaryAction",
    "outputTask",
    "composer",
    "input",
  ]) {
    if (requireAllHits) assert.ok(after[key].hitWithinTarget, `${key} is not hit-testable`);
    assert.notEqual(after[key].pointerEvents, "none", `${key} lost pointer events`);
  }
}

function colorAlpha(color) {
  const match = color.match(/rgba?\([^,]+,[^,]+,[^,]+(?:,\s*([0-9.]+))?\)/);
  return match?.[1] === undefined ? 1 : Number.parseFloat(match[1]);
}

function colorChannelAverage(color) {
  const channels = color
    .match(/[\d.]+/g)
    ?.slice(0, 3)
    .map(Number);
  return channels?.length === 3 ? channels.reduce((sum, channel) => sum + channel, 0) / 3 : 0;
}

async function captureStreamingAssistantSurface(targetPage) {
  return targetPage.evaluate(async () => {
    const conversation = document.querySelector('[data-codex-conversation="true"]');
    const unit = document.createElement("div");
    unit.id = "streaming-assistant-unit";
    unit.setAttribute("data-content-search-unit-key", "mock:streaming:assistant");
    const content = document.createElement("div");
    content.id = "streaming-assistant-content";
    content.textContent = "流式推理首帧也必须立即具备稳定阅读层。";
    unit.append(content);
    conversation.append(unit);

    const sample = () => getComputedStyle(content).backgroundColor;
    const inserted = sample();
    const firstFrame = await new Promise((resolve) => {
      requestAnimationFrame(() => resolve(sample()));
    });
    content.setAttribute("data-response-annotation-target", "mock-streaming-message");
    const annotated = sample();
    unit.remove();
    return { inserted, firstFrame, annotated };
  });
}

async function captureDynamicPreparationSurface(targetPage) {
  return targetPage.evaluate(async () => {
    const conversation = document.querySelector('[data-codex-conversation="true"]');
    const preparation = document.createElement("span");
    preparation.className = "loading-shimmer-pure-text";
    preparation.textContent = "Preparing a dynamically loaded skill";
    preparation.style.setProperty("color", "rgba(9,18,27,0.4)", "important");
    conversation.append(preparation);
    await Promise.resolve();
    await new Promise((resolve) => requestAnimationFrame(resolve));

    const themedStyle = getComputedStyle(preparation);
    const themed = {
      color: themedStyle.color,
      backgroundColor: themedStyle.backgroundColor,
      animationName: themedStyle.animationName,
    };
    preparation.remove();
    await Promise.resolve();

    return {
      themed,
      restoredInlineColor: preparation.style.getPropertyValue("color"),
      restoredInlinePriority: preparation.style.getPropertyPriority("color"),
      restoredInlineFill: preparation.style.getPropertyValue("-webkit-text-fill-color"),
      restoredInlineAnimation: preparation.style.getPropertyValue("animation"),
    };
  });
}

async function captureStreamingMutationCost(targetPage) {
  return targetPage.evaluate(async () => {
    const target = document.querySelector(
      '[data-content-search-unit-key="mock:assistant"]>[data-response-annotation-target]',
    );
    const originalGetComputedStyle = globalThis.getComputedStyle;
    const originalGetBoundingClientRect = Element.prototype.getBoundingClientRect;
    let styleReads = 0;
    let layoutReads = 0;
    globalThis.getComputedStyle = (...args) => {
      styleReads += 1;
      return originalGetComputedStyle(...args);
    };
    Element.prototype.getBoundingClientRect = function (...args) {
      layoutReads += 1;
      return originalGetBoundingClientRect.apply(this, args);
    };
    try {
      for (let index = 0; index < 24; index += 1) {
        target.append(document.createTextNode(` ${index}`));
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
      await new Promise((resolve) => requestAnimationFrame(resolve));
      return { styleReads, layoutReads };
    } finally {
      globalThis.getComputedStyle = originalGetComputedStyle;
      Element.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    }
  });
}

function assertAdaptiveSurfaceSystem(theme, state) {
  const { adaptiveTokens, statusStyles } = state;
  for (const [name, value] of Object.entries(adaptiveTokens)) {
    assert.notEqual(value, "", `${theme.id} did not emit adaptive token ${name}`);
  }
  assert.ok(
    colorAlpha(state.sidebarMaterial.backgroundColor) >= 0.74,
    `${theme.id} sidebar is too transparent`,
  );
  assert.ok(
    colorAlpha(state.outputMaterial.backgroundColor) >= 0.74,
    `${theme.id} output panel is too transparent`,
  );
  assert.ok(
    colorAlpha(state.composerMaterial.backgroundColor) >= 0.9,
    `${theme.id} composer is too transparent`,
  );
  assert.equal(state.sidebarMaterial.backdropFilter, "none");
  assert.equal(state.outputMaterial.backdropFilter, "none");
  assert.equal(state.composerMaterial.backdropFilter, "none");
  assert.equal(state.headerMaterial.backdropFilter, "none");
  assert.equal(state.outputMaterial.borderRadius, "16px");
  assert.equal(state.composerMaterial.borderRadius, "18px");
  assert.equal(state.userBubbleMaterial.borderRadius, "14px");
  assert.equal(statusStyles.running.borderLeftWidth, "3px");
  assert.equal(statusStyles.success.borderLeftWidth, "3px");
  assert.equal(statusStyles.warning.borderLeftWidth, "3px");
  assert.equal(statusStyles.error.borderLeftWidth, "3px");
  assert.equal(statusStyles.info.borderLeftWidth, "3px");
  assert.notEqual(statusStyles.running.borderLeftColor, statusStyles.success.borderLeftColor);
  assert.notEqual(statusStyles.success.borderLeftColor, statusStyles.warning.borderLeftColor);
  assert.notEqual(statusStyles.warning.borderLeftColor, statusStyles.error.borderLeftColor);
  for (const [status, style] of Object.entries(statusStyles)) {
    assert.ok(style.height >= 42, `${theme.id} ${status} status row is too cramped`);
    assert.ok(Number.parseFloat(style.marginTop) >= 6, `${theme.id} ${status} has no row spacing`);
    assert.ok(Number.parseFloat(style.paddingTop) >= 8, `${theme.id} ${status} lacks top padding`);
    assert.ok(
      Number.parseFloat(style.paddingBottom) >= 8,
      `${theme.id} ${status} lacks bottom padding`,
    );
  }
  assert.match(state.backdrop.filter, /brightness\([0-9.]+%?\)/);
  assert.match(state.backdrop.filter, /saturate\([0-9.]+%?\)/);
  assert.match(state.backdrop.filter, /contrast\([0-9.]+%?\)/);
  assert.doesNotMatch(state.backdrop.filter, /blur\(/);
}

function assertChromeReadability(theme, state) {
  for (const key of [
    "sidebarPrimary",
    "sidebarRoleButton",
    "composerPrimary",
    "menuPrimary",
    "outputPrimary",
    "statusPrimary",
  ]) {
    assert.match(
      state.chromeDescendants[key].color,
      /rgba?\(248, 249, 252/,
      `${theme.id} ${key} kept an unreadable official foreground token`,
    );
  }
  for (const key of ["headerMuted", "composerMuted"]) {
    assert.match(
      state.chromeDescendants[key].color,
      /rgba?\(232, 236, 243/,
      `${theme.id} ${key} kept an unreadable official muted token`,
    );
  }
  assert.equal(
    state.windowControl.color,
    "rgb(36, 42, 50)",
    `${theme.id} native titlebar control lost its dark foreground`,
  );
  assert.ok(
    colorAlpha(state.windowControl.backgroundColor) === 0 ||
      colorChannelAverage(state.windowControl.backgroundColor) >= 180,
    `${theme.id} native titlebar control kept an incompatible dark surface: ${state.windowControl.backgroundColor}`,
  );
}

try {
  await page.setContent(mockHtml, { waitUntil: "load" });
  await page.locator("#content-image").evaluate((image) => image.decode());
  const baseline = await page.evaluate(snapshotExpression());
  await page.screenshot({ path: path.join(outputDirectory, "01-baseline.png") });

  const firstTheme = sourceExport.themes[0];
  const secondTheme = sourceExport.themes[1];
  assert.equal(
    await page.evaluate((source) => globalThis.eval(source), firstTheme.application_source),
    true,
    "theme application source rejected the mock Codex DOM",
  );
  assert.equal(
    await page.evaluate((source) => globalThis.eval(source), firstTheme.verification_source),
    true,
    "theme verification source rejected the applied state",
  );
  const dynamicPreparationSurface = await captureDynamicPreparationSurface(page);
  assert.ok(
    colorAlpha(dynamicPreparationSurface.themed.backgroundColor) >= 0.74,
    `dynamic preparation surface is too transparent: ${dynamicPreparationSurface.themed.backgroundColor}`,
  );
  assert.match(
    dynamicPreparationSurface.themed.color,
    /rgba?\(248, 249, 252/,
    `dynamic preparation text is unreadable: ${dynamicPreparationSurface.themed.color}`,
  );
  assert.equal(dynamicPreparationSurface.themed.animationName, "none");
  assert.equal(dynamicPreparationSurface.restoredInlineColor, "rgba(9, 18, 27, 0.4)");
  assert.equal(dynamicPreparationSurface.restoredInlinePriority, "important");
  assert.equal(dynamicPreparationSurface.restoredInlineFill, "");
  assert.equal(dynamicPreparationSurface.restoredInlineAnimation, "");
  const streamingMutationCost = await captureStreamingMutationCost(page);
  assert.ok(
    streamingMutationCost.styleReads <= 8,
    `streaming content caused ${streamingMutationCost.styleReads} theme style reads`,
  );
  assert.ok(
    streamingMutationCost.layoutReads <= 8,
    `streaming content caused ${streamingMutationCost.layoutReads} theme layout reads`,
  );
  const streamingAssistantSurface = await captureStreamingAssistantSurface(page);
  assert.ok(
    colorAlpha(streamingAssistantSurface.inserted) >= 0.91,
    `streaming assistant surface is missing at insertion: ${streamingAssistantSurface.inserted}`,
  );
  assert.ok(
    colorAlpha(streamingAssistantSurface.firstFrame) >= 0.91,
    `streaming assistant surface is missing on the first frame: ${streamingAssistantSurface.firstFrame}`,
  );
  assert.ok(
    colorAlpha(streamingAssistantSurface.annotated) >= 0.91,
    `streaming assistant surface is missing after annotation: ${streamingAssistantSurface.annotated}`,
  );

  if (canary === "overlay") {
    await page.addStyleTag({
      content:
        "body::after{content:'';position:fixed;inset:0;z-index:2147483647;pointer-events:auto;background:rgba(255,0,0,.04)}",
    });
  }
  if (canary === "semantic-recolor") {
    await page.addStyleTag({ content: ".semantic-text{color:rgb(255,0,0)!important}" });
  }
  if (canary === "invisible-focus") {
    await page.locator("#composer-input").evaluate((element) => {
      element.style.setProperty("outline", "none", "important");
      element.style.setProperty("box-shadow", "none", "important");
    });
  }

  await page.waitForTimeout(220);
  const applied = await page.evaluate(snapshotExpression());
  await page.screenshot({
    path: path.join(outputDirectory, canary ? "canary-overlay.png" : "02-applied.png"),
  });
  if (captureGallery) {
    await page.screenshot({
      path: path.join(outputDirectory, `layout-${firstTheme.id}.png`),
    });
  }
  assertSemanticContentPreserved(baseline, applied);
  assert.equal(applied.styleCount, 1, "theme must own exactly one style element");
  assert.equal(applied.themeId, firstTheme.id, "wrong theme committed");
  assert.notEqual(
    applied.backdrop.backgroundImage,
    baseline.backdrop.backgroundImage,
    "backdrop did not change",
  );
  assert.notEqual(applied.mainBackground, baseline.mainBackground, "main surface did not change");
  assertBackdropLayout(firstTheme, applied.backdrop);
  assert.equal(applied.backdrop.backgroundAttachment, "fixed", "backdrop scrolls with content");
  assert.equal(applied.backdrop.pointerEvents, "none", "backdrop captures input");
  assert.equal(
    (applied.backdrop.backgroundImage.match(/data:image\//g) ?? []).length,
    1,
    "theme loads more than one background image",
  );
  assert.match(applied.scrim.backgroundImage, /linear-gradient/);
  assert.equal(applied.scrim.pointerEvents, "none", "readability scrim captures input");
  assert.equal(applied.mainBackground, "rgba(0, 0, 0, 0)");
  assert.equal(applied.mainBackgroundImage, "none");
  assertAdaptiveSurfaceSystem(firstTheme, applied);
  assertChromeReadability(firstTheme, applied);
  assert.match(applied.sidebarMaterial.color, /rgba?\(248, 249, 252/);
  assert.match(applied.headerIconFill, /rgba?\(248, 249, 252/);
  assert.equal(applied.titlebarMaterial.backgroundColor, "rgba(248, 249, 252, 0.94)");
  assert.equal(applied.titlebarMaterial.backdropFilter, "none");
  assert.equal(applied.titlebarMaterial.color, "rgb(36, 42, 50)");
  assert.ok(colorAlpha(applied.outputHeaderBackground) >= 0.85);
  assert.ok(colorAlpha(applied.assistantReadingBackground) >= 0.91);
  assert.ok(
    colorAlpha(applied.toolReadingBackground) >= 0.85,
    `status module background is too transparent: ${applied.toolReadingBackground}`,
  );
  assert.ok(
    colorAlpha(applied.streamingPreparation.backgroundColor) >= 0.74,
    `streaming preparation surface is too transparent: ${applied.streamingPreparation.backgroundColor}`,
  );
  assert.match(
    applied.streamingPreparation.color,
    /rgba?\(248, 249, 252/,
    `streaming preparation text is unreadable: ${applied.streamingPreparation.color}`,
  );
  assert.ok(
    applied.costlyGlassCount <= 3,
    `shared theme material created ${applied.costlyGlassCount} live glass layers`,
  );
  assert.ok(
    applied.costlyGlassPixelRatio <= 0.03,
    `shared theme material blurs ${(applied.costlyGlassPixelRatio * 100).toFixed(1)}% of the viewport`,
  );
  assert.ok(
    colorChannelAverage(applied.composerAttachment.backgroundColor) <= 90,
    `composer attachment kept an incompatible bright surface: ${applied.composerAttachment.backgroundColor}`,
  );
  assert.ok(
    colorChannelAverage(applied.composerAttachmentLabel.color) >= 180,
    `composer attachment label is unreadable: ${applied.composerAttachmentLabel.color}`,
  );
  assert.doesNotMatch(
    applied.composerFade.backgroundImage,
    /rgb\(255,\s*255,\s*255\)/,
    "composer retained the official white bottom fade",
  );

  await page.evaluate(() => {
    const main = document.querySelector("main.main-surface");
    const rect = main.getBoundingClientRect();
    const selectionOverlay = document.createElement("div");
    selectionOverlay.id = "selection-copy-overlay";
    selectionOverlay.setAttribute("data-selection-copy-overlay", "true");
    Object.assign(selectionOverlay.style, {
      position: "fixed",
      left: `${rect.left + rect.width / 2 - 80}px`,
      top: `${rect.top + rect.height / 2 - 40}px`,
      width: "160px",
      height: "80px",
      zIndex: "2147483647",
      pointerEvents: "auto",
      background: "transparent",
    });
    document.body.append(selectionOverlay);
  });
  await page.waitForTimeout(50);
  const selectionOverlayState = await page.evaluate(
    pageClassExpression(sourceExport.classification_source),
    sourceExport.classification_source,
  );
  assert.equal(
    selectionOverlayState.classification,
    "compatible-main",
    "a temporary selection/copy overlay invalidated the main task page",
  );
  assert.equal(
    selectionOverlayState.styleDisabled,
    false,
    "selecting text disabled the active theme",
  );
  assert.equal(
    selectionOverlayState.backdrop,
    applied.backdrop.backgroundImage,
    "selecting text removed the themed backdrop",
  );
  await page.locator("#selection-copy-overlay").evaluate((element) => element.remove());
  await page.waitForTimeout(50);

  await page.evaluate(() => {
    document.querySelector("[data-codex-conversation]").hidden = true;
    document.querySelector("[data-codex-home-state]").hidden = false;
  });
  await page.waitForTimeout(80);
  const homeState = await page.evaluate(snapshotExpression());
  await page.screenshot({ path: path.join(outputDirectory, "10-empty-home.png") });
  assert.equal(homeState.welcomeVisible, true, "empty new-task page is missing the welcome home");
  assert.equal(homeState.welcomeCardCount, 4, "welcome home must expose four bounded shortcuts");
  await page.locator('[data-codex-assistant-welcome-action="analyze-repository"]').click();
  assert.match(
    await page.locator("#composer-input").inputValue(),
    /分析/,
    "welcome shortcut did not safely prefill the native composer",
  );
  await page.evaluate(() => {
    document.querySelector("[data-codex-home-state]").hidden = true;
    document.querySelector("[data-codex-conversation]").hidden = false;
  });
  await page.waitForTimeout(80);
  assert.equal(
    (await page.evaluate(snapshotExpression())).welcomeVisible,
    false,
    "welcome home remained on an existing conversation",
  );
  await page.locator("#composer-input").fill("");

  await page.locator("#primary-action").click();
  await page.locator("#active-thread").click();
  await page.locator("#icon-button").click();
  await page.locator("#output-task").click();
  await page.locator("#menu-trigger").click();
  await page.getByRole("menuitem", { name: "菜单项" }).click();
  await page.locator("#dialog-trigger").click();
  assert.equal(await page.locator("#mock-dialog").evaluate((dialog) => dialog.open), true);
  await page.locator("#dialog-close").click();
  await page.locator("#native-dropdown").selectOption("two");
  await page.locator("#safe-link").click();
  await page.locator("#scroll-box").focus();
  await page.locator("#scroll-box").evaluate((element) => {
    element.scrollTop = 120;
  });
  assert.ok(await page.locator("#scroll-box").evaluate((element) => element.scrollTop > 0));
  await page.locator("#composer-input").fill("主题下仍能正常输入");
  await page.locator("#composer-input").press("End");
  await page.locator("#composer-input").pressSequentially("！");
  await page.locator("#composer-input").focus();
  const focusStyle = await page.locator("#composer-input").evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      outlineStyle: style.outlineStyle,
      outlineWidth: style.outlineWidth,
      boxShadow: style.boxShadow,
    };
  });
  assert.ok(
    (focusStyle.outlineStyle !== "none" && Number.parseFloat(focusStyle.outlineWidth) > 0) ||
      (focusStyle.boxShadow !== "none" && focusStyle.boxShadow !== ""),
    "focused composer has no visible focus indicator",
  );
  await page.locator("#send-button").click();
  assert.deepEqual(await page.evaluate(() => window.__mockEvents), {
    primary: 1,
    sidebar: 1,
    icon: 1,
    output: 1,
    menu: 1,
    dialog: 1,
    link: 1,
    sent: ["主题下仍能正常输入！"],
  });
  await page.evaluate(() => window.scrollTo(0, 0));

  const verifiedThemeIds = [firstTheme.id];
  for (const [index, theme] of sourceExport.themes.slice(1).entries()) {
    assert.equal(
      await page.evaluate((source) => globalThis.eval(source), theme.application_source),
      true,
      `${theme.id} application failed`,
    );
    assert.equal(
      await page.evaluate((source) => globalThis.eval(source), theme.verification_source),
      true,
      `${theme.id} verification failed`,
    );

    const switched = await page.evaluate(snapshotExpression());
    assertBackdropLayout(theme, switched.backdrop);
    assertChromeReadability(theme, switched);
    if (captureGallery) {
      await page.screenshot({
        path: path.join(outputDirectory, `layout-${theme.id}.png`),
      });
    }
    assertSemanticContentPreserved(baseline, switched);
    assert.equal(switched.styleCount, 1, `${theme.id} left multiple owned style elements`);
    assert.equal(switched.themeId, theme.id, `${theme.id} did not replace the active theme`);
    assert.notEqual(
      switched.backdrop.backgroundImage,
      baseline.backdrop.backgroundImage,
      `${theme.id} did not apply a backdrop`,
    );

    const ordinal = index + 2;
    await page.locator("#primary-action").click();
    await page.locator("#active-thread").click();
    await page.locator("#icon-button").click();
    await page.locator("#output-task").click();
    await page.locator("#composer-input").fill(`第${ordinal}套主题仍可输入并发送`);
    await page.locator("#send-button").click();
    const events = await page.evaluate(() => window.__mockEvents);
    assert.equal(events.primary, ordinal, `${theme.id} primary action did not receive a click`);
    assert.equal(events.sidebar, ordinal, `${theme.id} sidebar did not receive a click`);
    assert.equal(events.icon, ordinal, `${theme.id} icon button did not receive a click`);
    assert.equal(events.output, ordinal, `${theme.id} output panel did not receive a click`);
    assert.equal(
      events.sent.at(-1),
      `第${ordinal}套主题仍可输入并发送`,
      `${theme.id} composer did not submit the typed message`,
    );

    if (theme.id === secondTheme.id) {
      await page.screenshot({ path: path.join(outputDirectory, "03-switched.png") });
      assert.notEqual(
        switched.backdrop.backgroundImage,
        applied.backdrop.backgroundImage,
        "theme switch kept the old backdrop",
      );
    }
    verifiedThemeIds.push(theme.id);
  }

  const matrix = [];
  const adaptiveProfiles = new Set();
  await page.emulateMedia({ reducedMotion: "reduce" });
  const viewportFamilies = [
    { name: "full-hd", width: 1920, height: 1080 },
    { name: "qhd", width: 2560, height: 1440 },
    { name: "ultrawide", width: 3440, height: 1440 },
    { name: "windowed", width: 1280, height: 800 },
  ];
  const scales = [1, 1.25, 1.5, 2];
  for (const viewport of viewportFamilies) {
    for (const scale of scales) {
      await page.setViewportSize({
        width: Math.floor(viewport.width / scale),
        height: Math.floor(viewport.height / scale),
      });
      for (const theme of sourceExport.themes) {
        assert.equal(
          await page.evaluate((source) => globalThis.eval(source), theme.application_source),
          true,
          `${theme.id} failed at ${viewport.name} ${scale}`,
        );
        await page.waitForTimeout(20);
        const state = await page.evaluate(snapshotExpression());
        assertSemanticContentPreserved(baseline, state, false);
        assertBackdropLayout(theme, state.backdrop);
        assert.equal(state.backdrop.pointerEvents, "none", `${theme.id} background captured input`);
        assertAdaptiveSurfaceSystem(theme, state);
        assertChromeReadability(theme, state);
        adaptiveProfiles.add(
          `${state.adaptiveTokens.luminance}/${state.adaptiveTokens.complexity}/${state.adaptiveTokens.saturation}`,
        );
        assert.ok(
          state.costlyGlassCount <= 3,
          `${theme.id} created ${state.costlyGlassCount} live glass layers at ${viewport.name} ${scale}`,
        );
        assert.ok(
          state.costlyGlassPixelRatio <= 0.03,
          `${theme.id} blurs ${(state.costlyGlassPixelRatio * 100).toFixed(1)}% of the viewport at ${viewport.name} ${scale}`,
        );
        for (const selector of [
          "#active-thread",
          "#icon-button",
          "#primary-action",
          "#output-task",
          "#composer-input",
          "#send-button",
        ]) {
          const locator = page.locator(selector);
          await locator.scrollIntoViewIfNeeded();
          assert.equal(await locator.isEnabled(), true, `${theme.id} disabled ${selector}`);
        }
        matrix.push({
          themeId: theme.id,
          page: "main-task",
          viewport: viewport.name,
          width: Math.floor(viewport.width / scale),
          height: Math.floor(viewport.height / scale),
          scalePercent: Math.round(scale * 100),
          status: "passed",
        });
      }
      if (viewport.name === "windowed" && scale === 1) {
        await page.screenshot({ path: path.join(outputDirectory, "08-windowed.png") });
      }
      if (viewport.name === "ultrawide" && scale === 1) {
        await page.screenshot({ path: path.join(outputDirectory, "09-ultrawide.png") });
      }
    }
  }
  await page.setViewportSize({ width: 1440, height: 900 });
  assert.ok(
    adaptiveProfiles.size >= 3,
    "the all-theme render matrix did not exercise distinct adaptive background profiles",
  );
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await page.evaluate(() => window.scrollTo(0, 0));

  await page.evaluate((useCanary) => {
    const prompt = document.createElement("section");
    prompt.id = "security-prompt";
    prompt.setAttribute(useCanary ? "data-security-canary" : "data-security-prompt", "true");
    prompt.textContent = "安全确认保持官方外观";
    Object.assign(prompt.style, {
      position: "fixed",
      top: "90px",
      left: "50%",
      width: "320px",
      height: "80px",
      transform: "translateX(-50%)",
      background: "white",
      zIndex: "30",
    });
    document.querySelector("main.main-surface").append(prompt);
  }, canary === "page-class");
  await page.waitForTimeout(50);
  const sensitiveState = await page.evaluate(
    pageClassExpression(sourceExport.classification_source),
    sourceExport.classification_source,
  );
  await page.screenshot({ path: path.join(outputDirectory, "05-sensitive.png") });
  assert.equal(sensitiveState.classification, "sensitive", "security prompt was not classified");
  assert.equal(sensitiveState.styleDisabled, true, "sensitive page kept rich theme styling");
  assert.equal(sensitiveState.pageClass, null, "sensitive page kept the rich-theme marker");
  assert.equal(
    sensitiveState.backdrop,
    baseline.backdrop.backgroundImage,
    "sensitive page changed backdrop",
  );
  assert.equal(
    sensitiveState.mainBackground,
    baseline.mainBackground,
    "sensitive page changed surface",
  );

  await page.evaluate(() => {
    document.querySelector("#security-prompt").remove();
    globalThis.__detachedComposer = document.querySelector("#composer");
    globalThis.__detachedComposer.remove();
    const utility = document.createElement("section");
    utility.id = "utility-page";
    utility.setAttribute("data-page-kind", "sites");
    utility.innerHTML =
      '<div id="sites-sticky-fade" class="sticky z-30 bg-token-main-surface-primary after:pointer-events-none after:absolute after:top-full after:right-0 after:left-0 after:h-8 after:bg-linear-to-b after:from-token-main-surface-primary after:to-transparent after:content-[\'\']"><h2 id="sites-title">站点</h2><input id="sites-search" aria-label="搜索站点"><button id="sites-create" type="button">创建</button></div>';
    utility.style.minHeight = "240px";
    utility.querySelector("#sites-sticky-fade").style.position = "relative";
    document.querySelector("main.main-surface .content").hidden = true;
    document.querySelector("main.main-surface").append(utility);
  });
  await page.waitForTimeout(50);
  const utilityState = await page.evaluate(
    pageClassExpression(sourceExport.classification_source),
    sourceExport.classification_source,
  );
  await page.screenshot({ path: path.join(outputDirectory, "06-utility.png") });
  assert.equal(
    utilityState.classification,
    "compatible-shell",
    "sites shell was not classified as theme-compatible",
  );
  assert.equal(utilityState.styleDisabled, false, "sites shell lost the selected theme");
  assert.equal(utilityState.pageClass, "compatible-shell", "sites shell marker was not retained");
  assert.notEqual(
    utilityState.backdrop,
    baseline.backdrop.backgroundImage,
    "sites shell lost the themed backdrop",
  );
  assert.notEqual(
    utilityState.mainBackground,
    baseline.mainBackground,
    "sites shell lost the themed surface",
  );
  const utilityFade = await page.locator("#sites-sticky-fade").evaluate((element) => ({
    backgroundColor: getComputedStyle(element).backgroundColor,
    afterBackgroundImage: getComputedStyle(element, "::after").backgroundImage,
  }));
  assert.equal(
    utilityFade.backgroundColor,
    "rgba(0, 0, 0, 0)",
    "sites sticky header kept an opaque official surface",
  );
  assert.doesNotMatch(
    utilityFade.afterBackgroundImage,
    /rgb\(255,\s*255,\s*255\)/,
    "sites sticky header retained the official white fade",
  );
  for (const selector of ["#sites-title", "#sites-search", "#sites-create"]) {
    const locator = page.locator(selector);
    await locator.scrollIntoViewIfNeeded();
    const state = await locator.evaluate((element) => {
      const rect = element.getBoundingClientRect();
      const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      return {
        visible:
          getComputedStyle(element).visibility === "visible" && rect.width > 0 && rect.height > 0,
        hit: Boolean(hit && (hit === element || element.contains(hit))),
      };
    });
    assert.equal(state.visible, true, `${selector} is not visible on the themed sites shell`);
    assert.equal(state.hit, true, `${selector} is covered on the themed sites shell`);
  }

  await page.evaluate(() => {
    document.querySelector("#utility-page").remove();
    const settingsViewport = document.createElement("div");
    settingsViewport.className = "app-shell-main-content-viewport";
    const settingsPage = document.createElement("div");
    settingsPage.id = "settings-page";
    settingsPage.className = "main-surface flex h-full min-h-0 flex-col";
    settingsPage.innerHTML =
      '<button data-settings-panel-slug="general-settings" type="button">常规</button><div id="settings-main-surface"><h2 id="settings-title">常规</h2><div id="settings-card" class="flex flex-col overflow-hidden rounded-2xl border border-token-border"><label for="settings-toggle">自动审核</label><button id="settings-toggle" type="button" role="switch" aria-checked="true">已开启</button><button id="settings-select" class="bg-token-bg-fog border-token-border" type="button" aria-haspopup="listbox">PowerShell</button></div></div>';
    Object.assign(settingsPage.style, {
      minHeight: "720px",
      padding: "48px",
      background: "rgb(255, 255, 255)",
    });
    Object.assign(settingsPage.querySelector("#settings-card").style, {
      padding: "24px",
      background: "rgb(255, 255, 255)",
    });
    Object.assign(settingsPage.querySelector("#settings-select").style, {
      color: "rgb(248, 249, 252)",
      background: "rgba(255, 255, 255, 0.96)",
    });
    settingsViewport.append(settingsPage);
    document.querySelector("main.main-surface").append(settingsViewport);
  });
  await page.waitForTimeout(50);
  const settingsState = await page.evaluate(
    pageClassExpression(sourceExport.classification_source),
    sourceExport.classification_source,
  );
  const settingsVisual = await page.locator("#settings-page").evaluate((element) => {
    const style = getComputedStyle(element);
    const title = getComputedStyle(document.querySelector("#settings-title"));
    const card = getComputedStyle(document.querySelector("#settings-card"));
    const select = getComputedStyle(document.querySelector("#settings-select"));
    const control = document.querySelector("#settings-toggle");
    const controlRect = control.getBoundingClientRect();
    const hit = document.elementFromPoint(
      controlRect.left + controlRect.width / 2,
      controlRect.top + controlRect.height / 2,
    );
    return {
      backgroundColor: style.backgroundColor,
      cardBackgroundColor: card.backgroundColor,
      selectBackgroundColor: select.backgroundColor,
      color: title.color,
      hit: Boolean(hit && (hit === control || control.contains(hit))),
    };
  });
  await page.screenshot({ path: path.join(outputDirectory, "11-settings.png") });
  assert.equal(settingsState.classification, "compatible-shell");
  assert.equal(settingsState.styleDisabled, false, "settings shell lost the selected theme");
  assert.notEqual(
    settingsVisual.backgroundColor,
    "rgb(255, 255, 255)",
    "settings page retained its opaque official white surface",
  );
  assert.notEqual(
    settingsVisual.cardBackgroundColor,
    "rgb(255, 255, 255)",
    "settings card retained its opaque official white surface",
  );
  assert.notEqual(
    settingsVisual.selectBackgroundColor,
    "rgba(255, 255, 255, 0.96)",
    "settings selector kept a light surface behind light text",
  );
  assert.ok(
    colorChannelAverage(settingsVisual.color) >= 180,
    `settings text is unreadable: ${settingsVisual.color}`,
  );
  assert.equal(settingsVisual.hit, true, "settings theme covered the native control");

  await page.evaluate(() => document.querySelector("#settings-page").parentElement.remove());
  await page.waitForTimeout(50);
  const unknownState = await page.evaluate(
    pageClassExpression(sourceExport.classification_source),
    sourceExport.classification_source,
  );
  assert.equal(unknownState.classification, "unknown-build", "unknown page was misclassified");
  assert.equal(unknownState.styleDisabled, true, "unknown page kept rich theme styling");
  assert.equal(
    unknownState.backdrop,
    baseline.backdrop.backgroundImage,
    "unknown page changed backdrop",
  );

  await page.evaluate(() => {
    document.querySelector("main.main-surface .content").hidden = false;
    document.querySelector("main.main-surface").append(globalThis.__detachedComposer);
    delete globalThis.__detachedComposer;
  });
  await page.waitForTimeout(50);
  const returnedState = await page.evaluate(
    pageClassExpression(sourceExport.classification_source),
    sourceExport.classification_source,
  );
  assert.equal(returnedState.classification, "compatible-main", "main task did not recover");
  assert.equal(returnedState.styleDisabled, false, "main task theme did not recover");
  assert.equal(
    returnedState.styleCount,
    1,
    "route changes recreated or duplicated the theme style",
  );
  assert.equal(
    returnedState.backdrop,
    utilityState.backdrop,
    "route changes reloaded or replaced the background image",
  );
  assert.equal(
    returnedState.backdropPosition,
    utilityState.backdropPosition,
    "route changes moved the background focal point",
  );

  assert.equal(
    await page.evaluate(
      (source) => globalThis.eval(source),
      sourceExport.local_theme.application_source,
    ),
    true,
    "real local-import theme application failed",
  );
  assert.equal(
    await page.evaluate(
      (source) => globalThis.eval(source),
      sourceExport.local_theme.verification_source,
    ),
    true,
    "real local-import theme verification failed",
  );
  await page.locator("main .app-header-tint").scrollIntoViewIfNeeded();
  const imported = await page.evaluate(snapshotExpression());
  await page.screenshot({ path: path.join(outputDirectory, "07-local-import.png") });
  assertSemanticContentPreserved(baseline, imported);
  assert.equal(imported.themeId, sourceExport.local_theme.id, "local import did not own the theme");
  assert.equal(imported.styleCount, 1, "local import left multiple owned styles");

  assert.equal(
    await page.evaluate((source) => globalThis.eval(source), sourceExport.restore_source),
    true,
    "restore source failed",
  );
  const restored = await page.evaluate(snapshotExpression());
  await page.screenshot({ path: path.join(outputDirectory, "04-restored.png") });
  assertSemanticContentPreserved(baseline, restored);
  assert.equal(restored.styleCount, 0, "restore left an owned style element");
  assert.equal(restored.themeId, null, "restore left the theme API installed");
  assert.equal(
    restored.backdrop.backgroundImage,
    baseline.backdrop.backgroundImage,
    "restore left the themed backdrop",
  );
  assert.equal(
    restored.mainBackground,
    baseline.mainBackground,
    "restore left main surface styling",
  );

  const report = {
    status: "passed",
    themesGenerated: sourceExport.themes.length,
    themesVerified: verifiedThemeIds.length,
    verifiedThemeIds,
    appliedTheme: firstTheme.id,
    switchedTheme: secondTheme.id,
    localImportTheme: sourceExport.local_theme.id,
    pageClasses: {
      main: returnedState.classification,
      utility: utilityState.classification,
      settings: settingsState.classification,
      sensitive: sensitiveState.classification,
      unknown: unknownState.classification,
    },
    preserved: ["text", "content-image", "svg-fill", "primary-action-fill"],
    interactive: [
      "sidebar",
      "icon-button",
      "primary-action",
      "output-panel",
      "composer-input",
      "send",
      "menu",
      "dialog",
      "dropdown",
      "focus",
      "scroll",
      "link",
    ],
    restoreMatchedBaseline: true,
    matrix,
    matrixCases: matrix.length,
    scales: scales.map((scale) => Math.round(scale * 100)),
    viewportFamilies: viewportFamilies.map((viewport) => viewport.name),
    viewport: await page.evaluate(() => ({
      width: innerWidth,
      height: innerHeight,
      scrollWidth: document.documentElement.scrollWidth,
      scrollHeight: document.documentElement.scrollHeight,
    })),
  };
  await fs.writeFile(
    path.join(outputDirectory, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await browser.close();
}
