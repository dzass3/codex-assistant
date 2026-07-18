import { ProductDemo } from "./ProductDemo";

export default function Home() {
  return (
    <main>
      <header className="site-header">
        <a className="brand" href="#top" aria-label="Codex Assistant 首页">
          <span className="brand-mark" aria-hidden="true"><i /><i /><i /></span>
          <span>Codex Assistant</span>
        </a>
        <nav aria-label="主导航">
          <a href="#demo">产品演示</a>
          <a href="#privacy">隐私</a>
          <a href="#desktop">桌面版</a>
        </nav>
        <a className="header-cta" href="#demo">查看产品</a>
      </header>

      <section className="hero" id="top">
        <div className="hero-copy">
          <span className="eyebrow"><i /> WINDOWS-NATIVE CODEX COMPANION</span>
          <h1>让每个 Codex 子代理<br /><em>都用对模型。</em></h1>
          <p>
            在一个清晰的界面里观察原生子代理、验证真实有效模型，并按任务复杂度持续委派给
            Terra、Luna 与 Spark。质量优先，额度与时间其次。
          </p>
          <div className="hero-actions">
            <a className="primary-action" href="#demo">查看交互演示 <span>→</span></a>
            <a className="secondary-action" href="#desktop">了解桌面版边界</a>
          </div>
          <div className="trust-line">
            <span>✓ 不读取对话正文</span>
            <span>✓ 不保存推理与工具输出</span>
          </div>
        </div>
        <div className="hero-orbit" aria-hidden="true">
          <div className="orbit-ring orbit-ring--one" />
          <div className="orbit-ring orbit-ring--two" />
          <div className="model-core"><strong>Sol</strong><small>QUALITY ROOT</small></div>
          <div className="model-node node-terra"><strong>Terra</strong><small>复杂实现</small></div>
          <div className="model-node node-luna"><strong>Luna</strong><small>边界任务</small></div>
          <div className="model-node node-spark"><strong>Spark</strong><small>机械工作</small></div>
        </div>
      </section>

      <section className="metric-strip" aria-label="产品能力摘要">
        <div><strong>元数据限定</strong><span>隐私边界</span></div>
        <div><strong>2 层</strong><span>原生子代理深度</span></div>
        <div><strong>4 个模型层级</strong><span>质量优先路由</span></div>
        <div><strong>1 次点击</strong><span>版权核验主题</span></div>
      </section>

      <section className="demo-section" id="demo">
        <div className="section-heading">
          <div>
            <span className="eyebrow">ONE WINDOW · THREE CAPABILITIES</span>
            <h2>看得见，也调得动。</h2>
          </div>
          <p>公开网页仅展示脱敏示例数据；真实控制能力运行在本机 Windows 桌面版。</p>
        </div>
        <ProductDemo />
      </section>

      <section className="principles" id="privacy">
        <article>
          <span className="principle-number">01</span>
          <h3>有效模型，不是请求模型</h3>
          <p>同时展示 requested 与 effective model，只有经过原生层级和运行状态验证的模型才进入路由矩阵。</p>
        </article>
        <article>
          <span className="principle-number">02</span>
          <h3>质量门槛先于省额度</h3>
          <p>复杂后端与跨层任务保持 Terra 或 Sol；只有边界清晰、风险可控的工作才下沉到 Luna 与 Spark。</p>
        </article>
        <article>
          <span className="principle-number">03</span>
          <h3>本机元数据，严格最小化</h3>
          <p>只读取本地代理元数据，不采集提示词、回复、推理、命令参数、补丁内容或工具输出。</p>
        </article>
      </section>

      <section className="desktop-boundary" id="desktop">
        <div>
          <span className="eyebrow">NATIVE BY DESIGN</span>
          <h2>网页负责展示，桌面版负责控制。</h2>
          <p>
            控制本机 Codex 需要 Windows 桌面版。它验证 Microsoft Store 官方包、同一 Windows
            用户与回环端点后，才允许启动 Smart Routing、观察原生子代理或一键换肤。
          </p>
        </div>
        <div className="boundary-card">
          <span>Windows 11</span>
          <strong>Codex Assistant Desktop</strong>
          <ul>
            <li>原生子代理面板内持续路由</li>
            <li>安全重启与可恢复配置</li>
            <li>版权已核验的一键主题</li>
          </ul>
          <a
            className="download-action"
            href="/downloads/Codex-Assistant-0.5.0-x64-setup.exe"
            download
          >
            下载 Windows 安装包 <span>2.75 MiB</span>
          </a>
          <p>SHA-256 · f76c2aaf093a…11c724e</p>
        </div>
      </section>

      <footer>
        <a className="brand" href="#top"><span className="brand-mark" aria-hidden="true"><i /><i /><i /></span><span>Codex Assistant</span></a>
        <p>原生代理路由、模型观察与主题管理。</p>
        <span>Built for Windows · Metadata only</span>
      </footer>
    </main>
  );
}
