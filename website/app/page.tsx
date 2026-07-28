import Image from "next/image";
import type { CSSProperties } from "react";
import themeCatalog from "../../shared/theme-catalog.json";
import { HomepageMotion } from "./HomepageMotion";
import { ProductDemo } from "./ProductDemo";
import { releaseAssets, releaseTagUrl, releaseVersion } from "./release-data";

export default function Home() {
  return (
    <main className="site-shell">
      <HomepageMotion />
      <div className="hero-background" aria-hidden="true">
        <Image src="/images/observatory-hero.webp" alt="" fill priority unoptimized sizes="100vw" />
      </div>

      <header className="site-header load-in load-step-1">
        <a className="brand" href="#top" aria-label="Codex Assistant 首页">
          <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
          <span>Codex Assistant</span>
        </a>
        <nav aria-label="主导航">
          <a href="#demo">换肤演示</a>
          <a href="#safety">安全边界</a>
          <a href="#desktop">Windows 版</a>
        </nav>
        <a className="header-cta" data-spotlight href="#desktop">下载 {releaseVersion}</a>
      </header>

      <div className="hero-stage">
        <section className="hero" id="top">
          <div className="hero-copy">
            <span className="eyebrow load-in load-step-2"><i /> ONE-CLICK CODEX THEMES FOR WINDOWS</span>
            <h1 aria-label="给 Codex 换一套皮肤，不换掉它的能力。">
              <span className="title-line load-in load-step-3">给 Codex</span>
              <span className="title-line load-in load-step-4">换一套皮肤，</span>
              <em className="title-line load-in load-step-5">不换掉它的能力。</em>
            </h1>
            <p className="load-in load-step-6">
              选择版权已核验的内置主题，或导入自己的图片。Codex Assistant 只改变经过验证的视觉层，
              不覆盖文字、图标、输入区和原生交互。
            </p>
            <div className="hero-actions load-in load-step-7">
              <a className="primary-action" data-spotlight href="#desktop">下载 Windows 版 <span>→</span></a>
              <a className="secondary-action" data-spotlight href="#demo">查看安全换肤流程</a>
            </div>
            <div className="trust-line load-in load-step-8">
              <span>✓ 不修改官方安装文件</span>
              <span>✓ 本机图片不上传</span>
            </div>
          </div>
          <div className="theme-hero-preview load-in load-step-7" data-tilt="orbit" aria-label="主题效果示意">
            <div className="preview-window-bar"><i /><i /><i /><span>Codex · Roseglass Atelier</span></div>
            <div className="preview-app">
              <aside><strong>Codex</strong><span>新建任务</span><span>拉取请求</span><span>站点</span><span>插件</span><b>项目</b><span>我的项目</span></aside>
              <section>
                <small>ORIGINAL THEME · VERIFIED</small>
                <h2>视觉变了，能力还在。</h2>
                <p>背景、玻璃层与细节有层次；文字、图标和输入操作保持原生。</p>
                <div className="preview-cards"><i /><i /><i /></div>
                <div className="preview-composer"><span>随心输入…</span><b>↑</b></div>
              </section>
            </div>
          </div>
        </section>

        <section className="metric-strip" aria-label="产品能力摘要" data-reveal>
          <div data-spotlight><strong>{themeCatalog.themes.length} 个主题</strong><span>随安装包交付</span></div>
          <div data-spotlight><strong>本机导入</strong><span>PNG · JPEG · WebP</span></div>
          <div data-spotlight><strong>0 个官方文件</strong><span>不补丁、不改包</span></div>
          <div data-spotlight><strong>一键恢复</strong><span>随时回到官方外观</span></div>
        </section>
      </div>

      <section className="demo-section" id="demo" data-reveal>
        <div className="section-heading" data-reveal>
          <div><span className="eyebrow">ONE WINDOW · ONE PURPOSE</span><h2>只做换肤，把它做好。</h2></div>
          <p>{releaseVersion} 会先检测本机 Codex 环境，再把失败转成明确、可执行的处理指引。</p>
        </div>
        <ProductDemo />
      </section>

      <section className="principles" id="safety">
        <article data-reveal><span className="principle-number">01</span><h3>不遮文字与图标</h3><p>主题不覆盖语义前景色、主操作按钮或 SVG 填充；视觉层不接收鼠标事件。</p></article>
        <article data-reveal><span className="principle-number">02</span><h3>验证后才算成功</h3><p>主内容、侧栏和输入区必须可见且可点击；多页面部分失败会回到一致的官方外观。</p></article>
        <article data-reveal><span className="principle-number">03</span><h3>选择保留，应用由你决定</h3><p>主题选择会保留；完全关闭并从官方入口重新打开 ChatGPT/Codex 后，再到 Codex Assistant 点击一次“应用主题”。没有后台驻留或自动重启。</p></article>
      </section>

      <section className="theme-library" aria-labelledby="theme-library-title" data-reveal>
        <div className="section-heading">
          <div><span className="eyebrow">ONE MANIFEST · {themeCatalog.themes.length} RIGHTS-REVIEWED THEMES</span><h2 id="theme-library-title">每一套内置主题，都有权利清单。</h2></div>
          <p>只分发项目原创或已经人工核验商业再分发权的素材；你的本机图片永远不进入公开目录。</p>
        </div>
        <div className="theme-library-grid">
          {themeCatalog.themes.map((theme) => (
            <article key={theme.id} style={{ "--theme-accent": theme.palette.accent } as CSSProperties}>
              <i aria-hidden="true" /><div><strong>{theme.name}</strong><span>{theme.id}</span></div><b>已核验</b>
            </article>
          ))}
        </div>
      </section>

      <section className="desktop-boundary" id="desktop">
        <div data-reveal>
          <span className="eyebrow">NATIVE BY DESIGN</span>
          <h2>网页负责介绍，<br />桌面版负责换肤。</h2>
          <p>桌面版检测官方 Microsoft Store ChatGPT/Codex、窗口数量和主题会话。它不改变官方入口；每次完整重开后，由你回到 Codex Assistant 明确点击“应用主题”。</p>
        </div>
        <div className="boundary-card" data-reveal data-spotlight>
          <span>Windows 11 · current user install</span>
          <strong>Codex Assistant {releaseVersion}</strong>
          <ul>
            <li>{themeCatalog.themes.length} 个版权已核验的一键主题</li>
            <li>本机图片导入与哈希校验</li>
            <li>本机环境检测与具体修复指引</li>
            <li>官方入口不变，主题按需手动应用</li>
            <li>输入区、文字与图标可用性验证</li>
            <li>不兼容时恢复官方外观</li>
          </ul>
          <div className="download-grid" aria-label="Windows 安装包">
            {releaseAssets.map((asset) => (
              <a
                className="download-action"
                data-spotlight
                href={asset.url}
                key={asset.fileName}
              >
                <span>
                  <b>{asset.architecture} · {asset.format}</b>
                  <small>{asset.recommended ? "推荐安装方式" : "适合集中部署"}</small>
                </span>
                <em>{asset.sizeMiB} →</em>
              </a>
            ))}
          </div>
          <div className="unsigned-warning" role="note">
            <strong>未签名安装包</strong>
            <span>Windows 可能显示 SmartScreen 或“未知发布者”。请通过下方 SHA-256 校验下载。</span>
          </div>
          <div className="checksum-list" aria-label="安装包 SHA-256">
            {releaseAssets.map((asset) => (
              <p key={asset.fileName}>
                <span>{asset.fileName}</span>
                <code>{asset.sha256}</code>
              </p>
            ))}
          </div>
          <a className="release-link" href={releaseTagUrl}>查看完整 Release 与 SHA256SUMS.txt →</a>
        </div>
      </section>

      <footer data-reveal>
        <a className="brand" href="#top"><span className="brand-mark" aria-hidden="true"><i /><i /><i /></span><span>Codex Assistant</span></a>
        <p>安全、克制、不遮挡内容的 Codex 一键换肤。</p>
        <span>Built for Windows · Device only</span>
      </footer>
    </main>
  );
}
