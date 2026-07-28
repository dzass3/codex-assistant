"use client";

import Image from "next/image";
import { useState } from "react";

type DemoTab = "catalog" | "import" | "restore";

const tabs: Array<{ id: DemoTab; label: string }> = [
  { id: "catalog", label: "一键换肤" },
  { id: "import", label: "本机导入" },
  { id: "restore", label: "安全恢复" },
];

export function ProductDemo() {
  const [active, setActive] = useState<DemoTab>("catalog");

  return (
    <div className="product-window" data-tilt="window" data-spotlight data-reveal>
      <div className="window-bar">
        <div className="traffic-lights" aria-hidden="true"><i /><i /><i /></div>
        <strong>Codex Assistant</strong>
        <span className="connection"><i /> 仅在本机处理</span>
      </div>
      <div className="product-tabs" role="tablist" aria-label="主题功能演示">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={active === tab.id}
            aria-controls={`panel-${tab.id}`}
            onClick={() => setActive(tab.id)}
          >
            {tab.label}
          </button>
        ))}
        <span>公开网页仅展示界面，不控制本机 Codex</span>
      </div>

      <section id="panel-catalog" role="tabpanel" hidden={active !== "catalog"}>
        <div className="panel-heading">
          <div><small>RIGHTS-REVIEWED CATALOG</small><h3>选择主题，验证后再应用</h3></div>
          <span className="verified-pill">14 个内置主题</span>
        </div>
        <div className="theme-grid">
          <article className="demo-theme aurora-theme">
            <div className="theme-art"><span>AURORA GRID</span></div>
            <div><small>原创抽象</small><h4>Aurora Grid</h4><p>深海极光与克制的玻璃表面。</p><span className="rights-pill">版权已核验</span></div>
          </article>
          <article className="demo-theme muse-theme">
            <div className="theme-art"><Image src="/themes/observatory-muse.jpg" alt="Observatory Muse 原创角色主题" fill unoptimized sizes="(max-width: 650px) 100vw, 30vw" /></div>
            <div><small>原创角色</small><h4>Observatory Muse</h4><p>安静的紫色未来观测站。</p><span className="rights-pill">版权已核验</span></div>
          </article>
        </div>
      </section>

      <section id="panel-import" role="tabpanel" hidden={active !== "import"}>
        <div className="panel-heading">
          <div><small>DEVICE-ONLY IMPORT</small><h3>把自己的图片变成本机主题</h3></div>
          <span className="verified-pill">不会上传</span>
        </div>
        <div className="demo-flow">
          <FlowStep number="01" title="选择图片" text="支持真实 PNG、JPEG 与 WebP 文件。" />
          <FlowStep number="02" title="严格校验" text="检查签名、MIME、尺寸、大小与 SHA-256。" />
          <FlowStep number="03" title="只存本机" text="本机主题不会进入安装包或公开网站。" />
        </div>
        <div className="quality-banner"><strong>隐私边界</strong><span>图片字节不离开当前 Windows 设备，相同内容重复导入不会产生副本。</span></div>
      </section>

      <section id="panel-restore" role="tabpanel" hidden={active !== "restore"}>
        <div className="panel-heading">
          <div><small>FAIL-CLOSED RESTORE</small><h3>不兼容就回到官方外观</h3></div>
          <span className="verified-pill">不修改官方文件</span>
        </div>
        <div className="restore-proof">
          <div><span>主题样式</span><strong>单一自有节点</strong><small>只移除 Codex Assistant 创建的内容</small></div>
          <div><span>功能保护</span><strong>输入区可点击</strong><small>文字、按钮和图标保留 Codex 原生语义</small></div>
          <div><span>失败策略</span><strong>官方外观</strong><small>多页面验证失败不会误报为已应用</small></div>
        </div>
      </section>
      <div className="native-note">真正的一键换肤只在 Windows 桌面版中运行</div>
    </div>
  );
}

function FlowStep({ number, title, text }: { number: string; title: string; text: string }) {
  return <article><span>{number}</span><strong>{title}</strong><p>{text}</p></article>;
}
