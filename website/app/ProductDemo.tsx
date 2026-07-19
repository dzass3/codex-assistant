"use client";

import { useState } from "react";
import Image from "next/image";

type DemoTab = "agents" | "routing" | "themes";

const tabs: Array<{ id: DemoTab; label: string }> = [
  { id: "agents", label: "实时代理" },
  { id: "routing", label: "Smart Routing" },
  { id: "themes", label: "主题管理" },
];

export function ProductDemo() {
  const [active, setActive] = useState<DemoTab>("agents");
  return (
    <div className="product-window" data-tilt="window" data-spotlight data-reveal>
      <div className="window-bar">
        <div className="traffic-lights" aria-hidden="true"><i /><i /><i /></div>
        <strong>Codex Assistant</strong>
        <span className="connection"><i /> 已连接</span>
      </div>
      <div className="product-tabs" role="tablist" aria-label="产品功能演示">
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
        <span>仅展示脱敏示例数据</span>
      </div>

      <section id="panel-agents" role="tabpanel" hidden={active !== "agents"}>
        <div className="panel-heading"><div><small>AGENT TREE</small><h3>当前任务与子代理</h3></div><span>4 个代理 · 3 个运行中</span></div>
        <div className="agent-tree">
          <AgentRow depth={0} name="Codex Assistant 产品迭代" role="根任务" requested="GPT-5.6 Sol" effective="GPT-5.6 Sol" status="运行中" />
          <AgentRow depth={1} name="Smart Routing 跨层实现" role="实现代理" requested="GPT-5.6 Terra" effective="GPT-5.6 Terra" status="运行中" />
          <AgentRow depth={2} name="严格 IPC 契约" role="边界任务" requested="GPT-5.6 Luna" effective="GPT-5.6 Luna" status="已验证" />
          <AgentRow depth={2} name="主题元数据整理" role="机械任务" requested="GPT-5.3 Codex Spark" effective="GPT-5.3 Codex Spark" status="已验证" />
        </div>
      </section>

      <section id="panel-routing" role="tabpanel" hidden={active !== "routing"}>
        <div className="panel-heading"><div><small>QUALITY-FIRST MATRIX</small><h3>任务复杂度决定最低质量层级</h3></div><span className="verified-pill">6/6 原生能力已验证</span></div>
        <div className="routing-matrix">
          <div className="matrix-head"><span>任务类型</span><span>分配模型</span><span>质量门槛</span><span>状态</span></div>
          <RouteRow task="机械整理 / 精确改名" model="Spark" floor="低风险、边界固定" tone="spark" />
          <RouteRow task="单模块实现 / 只读评审" model="Luna" floor="边界清晰" tone="luna" />
          <RouteRow task="复杂后端 / 跨层任务" model="Terra" floor="跨文件与集成" tone="terra" />
          <RouteRow task="架构、高风险与最终把关" model="Sol" floor="最高质量" tone="sol" />
        </div>
        <div className="quality-banner"><strong>质量优先</strong><span>低模型无法满足质量底线时会自动升级，而不会为了省额度强行下沉。</span></div>
      </section>

      <section id="panel-themes" role="tabpanel" hidden={active !== "themes"}>
        <div className="panel-heading"><div><small>RIGHTS-AUDITED THEMES</small><h3>声明式一键换肤</h3></div><span>只分发原创素材</span></div>
        <div className="theme-grid">
          <article className="demo-theme aurora-theme"><div className="theme-art"><span>AURORA GRID</span></div><div><small>原创抽象</small><h4>Aurora Grid</h4><p>深海极光与克制的玻璃表面。</p><span className="rights-pill">版权已核验</span></div></article>
          <article className="demo-theme muse-theme"><div className="theme-art"><Image src="/themes/observatory-muse.jpg" alt="Observatory Muse 原创角色主题" fill unoptimized sizes="(max-width: 650px) 100vw, 30vw" /></div><div><small>原创角色</small><h4>Observatory Muse</h4><p>安静的紫色未来观测站。</p><span className="rights-pill">版权已核验</span></div></article>
        </div>
      </section>
      <div className="native-note">控制本机 Codex 需要 Windows 桌面版</div>
    </div>
  );
}

function AgentRow({ depth, name, role, requested, effective, status }: { depth: number; name: string; role: string; requested: string; effective: string; status: string }) {
  return <div className={`agent-row agent-row--depth-${depth}`}><span className="tree-mark" aria-hidden="true" /><div><strong>{name}</strong><small>{role}</small></div><div className="model-stack"><small>REQUESTED</small><span>{requested}</span></div><div className="model-stack"><small>EFFECTIVE</small><span>{effective}</span></div><b>{status}</b></div>;
}

function RouteRow({ task, model, floor, tone }: { task: string; model: string; floor: string; tone: string }) {
  return <div className="matrix-row"><strong>{task}</strong><span className={`tier tier--${tone}`}>{model}</span><span>{floor}</span><b>可用</b></div>;
}
