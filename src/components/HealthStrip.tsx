import type { MonitorSnapshot } from "../../shared/monitor-types";

export function HealthStrip({ health }: { health: MonitorSnapshot["health"] }) {
  const entries = [
    ["状态数据库", health.state_database],
    ["运行记录观察器", health.rollout_observer],
  ] as const;
  const hasIssue = entries.some(([, entry]) => entry.level !== "healthy");

  return (
    <section className={`health-strip ${hasIssue ? "health-strip--attention" : ""}`}>
      <div className="health-title">
        <span className={`health-orb ${hasIssue ? "health-orb--warn" : ""}`} />
        <strong>{hasIssue ? "部分数据源正在降级运行" : "监控数据源正常"}</strong>
      </div>
      <div className="health-sources">
        {entries.map(([label, entry]) => (
          <span key={label} title={entry.message}>
            <i className={`source-dot source-dot--${entry.level}`} />
            {label}
          </span>
        ))}
      </div>
    </section>
  );
}
