import { useMemo, useState } from "react";
import { AgentTree } from "./components/AgentTree";
import { FilterBar, type MonitorFilters } from "./components/FilterBar";
import { HealthStrip } from "./components/HealthStrip";
import { SettingsDialog } from "./components/SettingsDialog";
import { useMonitor } from "./hooks/useMonitor";

const DEFAULT_FILTERS: MonitorFilters = {
  query: "",
  model: "all",
  project: "all",
  activeOnly: true,
};

export function App() {
  const monitor = useMonitor();
  const [filters, setFilters] = useState(DEFAULT_FILTERS);
  const [settingsOpen, setSettingsOpen] = useState(false);

  const options = useMemo(() => {
    const agents = monitor.snapshot?.agents ?? [];
    return {
      models: [
        ...new Set(agents.map((agent) => agent.effective_model).filter(Boolean) as string[]),
      ].toSorted(),
      projects: [
        ...new Set(agents.map((agent) => agent.project).filter(Boolean) as string[]),
      ].toSorted(),
    };
  }, [monitor.snapshot]);

  const counts = monitor.snapshot?.counts;
  const lastUpdated = monitor.snapshot
    ? new Date(monitor.snapshot.generated_at_ms).toLocaleTimeString("zh-CN", {
        hour12: false,
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })
    : "--:--:--";

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <div>
            <h1>Codex Agent Monitor</h1>
            <p>子代理模型实时观察器</p>
          </div>
        </div>
        <div className="topbar-actions">
          <span className="last-updated">
            <i className={monitor.connected ? "pulse-dot" : "pulse-dot pulse-dot--off"} />
            {monitor.connected ? `已连接 · ${lastUpdated}` : "正在连接"}
          </span>
          <button className="icon-button" onClick={monitor.refresh} disabled={monitor.refreshing}>
            {monitor.refreshing ? "刷新中" : "立即刷新"}
          </button>
          <button className="icon-button" onClick={() => setSettingsOpen(true)}>
            设置
          </button>
        </div>
      </header>

      <section className="summary-grid" aria-label="监控摘要">
        <SummaryCard label="根任务" value={counts?.roots ?? 0} tone="neutral" />
        <SummaryCard label="子代理" value={counts?.subagents ?? 0} tone="blue" />
        <SummaryCard
          label="正在运行"
          value={(counts?.running ?? 0) + (counts?.starting ?? 0)}
          tone="green"
        />
        <SummaryCard label="可继续调用" value={counts?.idle ?? 0} tone="amber" />
        <SummaryCard label="模型漂移" value={counts?.model_drifts ?? 0} tone="violet" />
      </section>

      {monitor.snapshot && <HealthStrip health={monitor.snapshot.health} />}
      {monitor.error && <div className="global-error">{monitor.error}</div>}

      <section className="workspace">
        <FilterBar
          filters={filters}
          models={options.models}
          projects={options.projects}
          onChange={setFilters}
        />
        <div className="workspace-heading">
          <div>
            <span className="eyebrow">AGENT TREE</span>
            <h2>当前任务与子代理</h2>
          </div>
          <span className="privacy-note">只读元数据 · 不采集对话内容</span>
        </div>
        {monitor.loading ? (
          <LoadingState />
        ) : (
          <AgentTree agents={monitor.snapshot?.agents ?? []} filters={filters} />
        )}
      </section>

      <footer>
        <span>数据源：Codex 本地状态库与 rollout 元数据</span>
        <span>{monitor.settings?.codex_home_label ?? "Codex Home: 检测中"}</span>
      </footer>

      <SettingsDialog
        open={settingsOpen}
        settings={monitor.settings}
        onClose={() => setSettingsOpen(false)}
        onSave={monitor.setCodexHome}
      />
    </main>
  );
}

function SummaryCard({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone: "neutral" | "blue" | "green" | "amber" | "violet";
}) {
  return (
    <article className={`summary-card summary-card--${tone}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </article>
  );
}

function LoadingState() {
  return (
    <div className="empty-state">
      <span className="loading-ring" />
      <h3>正在建立只读观察</h3>
      <p>正在读取代理关系和有效模型元数据…</p>
    </div>
  );
}
