import { useMemo, useState } from "react";
import { useMonitor } from "../hooks/useMonitor";
import { AgentTree, type MonitorFilters } from "./AgentTree";
import { FilterBar } from "./FilterBar";
import { HealthStrip } from "./HealthStrip";
import { SettingsDialog } from "./SettingsDialog";

const DEFAULT_FILTERS: MonitorFilters = {
  query: "",
  model: "all",
  project: "all",
  activeOnly: true,
};

export function MonitorPage() {
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
  const observerStatus = monitor.snapshot?.observer_status ?? "error";
  const observerLabel = {
    live: "实时",
    delayed: "更新延迟",
    uncertain: "状态待确认",
    error: "监控异常",
  }[observerStatus];

  return (
    <section className="monitor-page" aria-labelledby="monitor-title">
      <div className="monitor-page__heading">
        <div>
          <span className="eyebrow">LOCAL · READ ONLY · METADATA</span>
          <h2 id="monitor-title">实时代理</h2>
          <p>查看原生子代理层级、实际模型和运行状态，不读取对话内容。</p>
        </div>
        <div className="monitor-page__actions">
          <span className="last-updated">
            <i className={monitor.connected ? "pulse-dot" : "pulse-dot pulse-dot--off"} />
            {monitor.connected ? `${observerLabel} · ${lastUpdated}` : "正在连接"}
          </span>
          <button
            type="button"
            className="button-secondary"
            onClick={() => void monitor.refresh()}
            disabled={monitor.refreshing}
          >
            {monitor.refreshing ? "刷新中" : "立即刷新"}
          </button>
          <button type="button" className="button-secondary" onClick={() => setSettingsOpen(true)}>
            设置
          </button>
        </div>
      </div>

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

      {monitor.snapshot ? <HealthStrip health={monitor.snapshot.health} /> : null}
      {monitor.error ? <div className="global-error">{monitor.error}</div> : null}

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
          <AgentTree
            agents={monitor.snapshot?.agents ?? []}
            filters={filters}
            codexRunning={monitor.snapshot?.codex_running ?? false}
          />
        )}
      </section>

      <footer className="monitor-footer">
        <span>数据源：Codex 本地状态库与 rollout 元数据</span>
        <span>{monitor.settings?.codex_home_label ?? "Codex Home: 检测中"}</span>
      </footer>
      <SettingsDialog
        open={settingsOpen}
        settings={monitor.settings}
        onClose={() => setSettingsOpen(false)}
        onSave={monitor.setCodexHome}
      />
    </section>
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
