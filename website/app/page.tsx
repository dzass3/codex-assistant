import Image from "next/image";
import type { CSSProperties } from "react";
import { HomepageMotion } from "./HomepageMotion";
import { ProductDemo } from "./ProductDemo";
import themeCatalog from "../../shared/theme-catalog.json";

export default function Home() {
  return (
    <main className="site-shell">
      <HomepageMotion />
      <div className="hero-background" aria-hidden="true">
        <Image
          src="/images/observatory-hero.webp"
          alt=""
          fill
          priority
          unoptimized
          sizes="100vw"
        />
      </div>

      <header className="site-header load-in load-step-1">
        <a className="brand" href="#top" aria-label="Codex Assistant 首页">
          <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
          <span>Codex Assistant</span>
        </a>
        <nav aria-label="主导航">
          <a href="#demo">产品演示</a>
          <a href="#privacy">隐私</a>
          <a href="#desktop">桌面版</a>
        </nav>
        <a className="header-cta" data-spotlight href="#demo">查看产品</a>
      </header>

      <div className="hero-stage">
        <section className="hero" id="top">
          <div className="hero-copy">
            <span className="eyebrow load-in load-step-2"><i /> WINDOWS-NATIVE CODEX COMPANION</span>
            <h1 aria-label="让每个 Codex 子代理 都用对模型。">
              <span className="title-line load-in load-step-3">让每个 Codex</span>
              <span className="title-line load-in load-step-4">子代理</span>
              <em className="title-line load-in load-step-5">都用对模型。</em>
            </h1>
            <p className="load-in load-step-6">
              在一个清晰的界面里观察原生子代理、验证真实有效模型，并按任务复杂度持续委派给
              Terra、Luna 与 Spark。质量优先，额度与时间其次。
            </p>
            <div className="hero-actions load-in load-step-7">
              <a className="primary-action" data-spotlight href="#demo">查看交互演示 <span>→</span></a>
              <a className="secondary-action" data-spotlight href="#desktop">了解桌面版边界</a>
            </div>
            <div className="trust-line load-in load-step-8">
              <span>✓ 不读取对话正文</span>
              <span>✓ 不保存推理与工具输出</span>
            </div>
          </div>
          <div className="orbit-shell load-in load-step-7">
            <div className="hero-orbit" data-active-model="" data-tilt="orbit" aria-hidden="true">
              <div className="orbit-glow" />
              <div className="orbit-ring orbit-ring--inner" />
              <div className="orbit-ring orbit-ring--middle" />
              <div className="orbit-ring orbit-ring--outer" />
              <div className="energy-path energy-path--one"><i /></div>
              <div className="energy-path energy-path--two"><i /></div>
              <div className="model-core"><strong>Sol</strong><small>QUALITY ROOT</small></div>
              <div className="model-node node-terra" data-model="terra"><strong>Terra</strong><small>复杂实现</small></div>
              <div className="model-node node-luna" data-model="luna"><strong>Luna</strong><small>边界任务</small></div>
              <div className="model-node node-spark" data-model="spark"><strong>Spark</strong><small>机械工作</small></div>
            </div>
          </div>
        </section>

        <section className="metric-strip" aria-label="产品能力摘要" data-reveal>
          <div data-spotlight><strong>元数据限定</strong><span>隐私边界</span></div>
          <div data-spotlight><strong><span data-count-to="2">2</span> 层</strong><span>原生子代理深度</span></div>
          <div data-spotlight><strong><span data-count-to="4">4</span> 个模型层级</strong><span>质量优先路由</span></div>
          <div data-spotlight><strong>12 个主题</strong><span>原创且版权核验</span></div>
        </section>
      </div>

      <section className="demo-section" id="demo" data-reveal>
        <div className="section-heading" data-reveal>
          <div>
            <span className="eyebrow">ONE WINDOW · THREE CAPABILITIES</span>
            <h2>看得见，也调得动。</h2>
          </div>
          <p>公开网页仅展示脱敏示例数据；真实控制能力运行在本机 Windows 桌面版。</p>
        </div>
        <ProductDemo />
      </section>

      <section className="principles" id="privacy">
        <article data-reveal>
          <span className="principle-number">01</span>
          <h3>有效模型，不是请求模型</h3>
          <p>同时展示 requested 与 effective model，只有经过原生层级和运行状态验证的模型才进入路由矩阵。</p>
        </article>
        <article data-reveal>
          <span className="principle-number">02</span>
          <h3>质量门槛先于省额度</h3>
          <p>复杂后端与跨层任务保持 Terra 或 Sol；只有边界清晰、风险可控的工作才下沉到 Luna 与 Spark。</p>
        </article>
        <article data-reveal>
          <span className="principle-number">03</span>
          <h3>本机元数据，严格最小化</h3>
          <p>只读取本地代理元数据，不采集提示词、回复、推理、命令参数、补丁内容或工具输出。</p>
        </article>
      </section>

      <section className="theme-library" aria-labelledby="theme-library-title" data-reveal>
        <div className="section-heading">
          <div>
            <span className="eyebrow">ONE MANIFEST · {themeCatalog.themes.length} RIGHTS-REVIEWED THEMES</span>
            <h2 id="theme-library-title">一套清单，桌面端与网站同步。</h2>
          </div>
          <p>全部主题均为项目原创或已完成商业再分发人工复核；不远程加载脚本、人物 IP 或仓库截图。</p>
        </div>
        <div className="theme-library-grid">
          {themeCatalog.themes.map((theme) => (
            <article key={theme.id} style={{ "--theme-accent": theme.palette.accent } as CSSProperties}>
              <i aria-hidden="true" />
              <div>
                <strong>{theme.name}</strong>
                <span>{theme.id}</span>
              </div>
              <b>已核验</b>
            </article>
          ))}
        </div>
      </section>

      <section className="desktop-boundary" id="desktop">
        <div data-reveal>
          <span className="eyebrow">NATIVE BY DESIGN</span>
          <h2>网页负责展示，桌面版负责<br />控制。</h2>
          <p>
            控制本机 Codex 需要 Windows 桌面版。它验证 Microsoft Store 官方包、同一 Windows
            用户与回环端点后，才允许启动 Smart Routing、观察原生子代理或一键换肤。
          </p>
        </div>
        <div className="boundary-card" data-reveal data-spotlight>
          <span>Windows 11</span>
          <strong>Codex Assistant Desktop</strong>
          <ul>
            <li>每个根任务独立开关并同步原生输入框</li>
            <li>安全重启与票据化受控强制重启</li>
            <li>12 个版权已核验的一键主题</li>
          </ul>
          <a
            className="download-action"
            data-spotlight
            href="/downloads/Codex-Assistant-0.7.3-x64-setup.exe"
            download
          >
            下载 Windows 安装包 <span>3.63 MiB</span>
          </a>
          <p>0.7.3 · 3,803,674 bytes · SHA-256 · 10321fb01959…9a31d97</p>
        </div>
      </section>

      <footer data-reveal>
        <a className="brand" href="#top"><span className="brand-mark" aria-hidden="true"><i /><i /><i /></span><span>Codex Assistant</span></a>
        <p>原生代理路由、模型观察与主题管理。</p>
        <span>Built for Windows · Metadata only</span>
      </footer>
    </main>
  );
}
