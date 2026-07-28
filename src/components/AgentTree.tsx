import { useMemo, type CSSProperties } from "react";
import type { AgentObservation, AgentStatus } from "../../shared/monitor-types";

const STATUS_LABELS: Record<AgentStatus, string> = {
  starting: "启动中",
  running: "运行中",
  uncertain: "状态待确认",
  "historical-unclosed": "历史状态未闭合",
  idle: "可继续调用",
  interrupted: "已中断",
  "tracking-error": "跟踪异常",
};

const SOURCE_LABELS = {
  "turn-context": "运行确认",
  "state-database": "状态库",
  "requested-only": "仅请求值",
  unknown: "未知来源",
} as const;

export interface MonitorFilters {
  query: string;
  model: string;
  project: string;
  activeOnly: boolean;
}

export function AgentTree({
  agents,
  filters,
  codexRunning = true,
}: {
  agents: AgentObservation[];
  filters: MonitorFilters;
  codexRunning?: boolean;
}) {
  const visibleAgents = useMemo(() => filterWithAncestors(agents, filters), [agents, filters]);
  const visibleIds = new Set(visibleAgents.map((agent) => agent.thread_id));
  const childMap = new Map<string | null, AgentObservation[]>();

  for (const agent of visibleAgents) {
    const parent =
      agent.parent_thread_id && visibleIds.has(agent.parent_thread_id)
        ? agent.parent_thread_id
        : null;
    const siblings = childMap.get(parent) ?? [];
    siblings.push(agent);
    childMap.set(parent, siblings);
  }
  for (const siblings of childMap.values()) {
    siblings.sort((left, right) => (right.updated_at_ms ?? 0) - (left.updated_at_ms ?? 0));
  }

  if (visibleAgents.length === 0) {
    return (
      <div className="empty-state">
        <span className="empty-glyph" aria-hidden="true">
          ◎
        </span>
        <h3>
          {!codexRunning
            ? "Codex 未运行"
            : agents.length === 0
              ? "暂未发现 Codex 任务"
              : "没有匹配的代理"}
        </h3>
        <p>
          {!codexRunning
            ? "历史记录不会被推断为当前活动；启动官方 Codex 后会自动恢复观察。"
            : agents.length === 0
              ? "启动一个包含子代理的 Codex 任务后，这里会自动出现。"
              : "调整筛选条件查看其他代理。"}
        </p>
      </div>
    );
  }

  return (
    <div className="agent-tree">
      {(childMap.get(null) ?? []).map((agent) => (
        <AgentBranch key={agent.thread_id} agent={agent} childMap={childMap} ancestry={new Set()} />
      ))}
    </div>
  );
}

function AgentBranch({
  agent,
  childMap,
  ancestry,
}: {
  agent: AgentObservation;
  childMap: Map<string | null, AgentObservation[]>;
  ancestry: Set<string>;
}) {
  if (ancestry.has(agent.thread_id)) return null;
  const nextAncestry = new Set(ancestry).add(agent.thread_id);
  const descendants = childMap.get(agent.thread_id) ?? [];

  return (
    <div className="agent-branch" style={{ "--depth": agent.depth } as CSSProperties}>
      <AgentRow agent={agent} childCount={descendants.length} />
      {descendants.length > 0 ? (
        <div className="agent-children">
          {descendants.map((child) => (
            <AgentBranch
              key={child.thread_id}
              agent={child}
              childMap={childMap}
              ancestry={nextAncestry}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function AgentRow({ agent, childCount }: { agent: AgentObservation; childCount: number }) {
  const age = formatAge(agent.freshness_ms);
  const effectiveModel = agent.effective_model ?? "尚未确认";

  return (
    <article
      className={`agent-row agent-row--${agent.status}`}
      data-testid={`agent-${agent.thread_id}`}
      title={formatFullTime(agent.updated_at_ms)}
    >
      <div className="agent-rail">
        <span className={`status-light status-light--${agent.status}`} />
      </div>
      <div className="agent-identity">
        <div className="agent-name-line">
          <strong>{agent.display_name}</strong>
          {agent.is_subagent ? <span className="subagent-tag">子代理</span> : null}
          {childCount > 0 ? <span className="child-count">{childCount} 个下级</span> : null}
        </div>
        <p>{[agent.role, agent.project, age].filter(Boolean).join(" · ") || "任务元数据待更新"}</p>
      </div>
      <div className="model-column">
        <div className="model-line">
          <span className="model-badge">{effectiveModel}</span>
          {agent.reasoning_effort ? (
            <span className="effort-badge">{agent.reasoning_effort}</span>
          ) : null}
        </div>
        <small>{SOURCE_LABELS[agent.model_source]}</small>
        {!agent.effective_model && agent.requested_model ? (
          <small className="requested-model">请求 {agent.requested_model}</small>
        ) : null}
      </div>
      <div className="agent-status-column">
        <span className={`status-pill status-pill--${agent.status}`}>
          {STATUS_LABELS[agent.status]}
        </span>
        {agent.model_drift ? (
          <div
            className="drift-badge"
            title={`请求 ${agent.requested_model ?? "未知"}，实际 ${effectiveModel}`}
          >
            <span>模型漂移</span>
            <small>
              {agent.requested_model} → {effectiveModel}
            </small>
          </div>
        ) : null}
      </div>
    </article>
  );
}

function filterWithAncestors(agents: AgentObservation[], filters: MonitorFilters) {
  const byId = new Map(agents.map((agent) => [agent.thread_id, agent]));
  const query = filters.query.trim().toLocaleLowerCase();
  const selected = new Set<string>();

  for (const agent of agents) {
    const haystack = [
      agent.display_name,
      agent.role,
      agent.project,
      agent.effective_model,
      agent.requested_model,
    ]
      .filter(Boolean)
      .join(" ")
      .toLocaleLowerCase();
    const active =
      agent.status === "running" ||
      agent.status === "starting" ||
      agent.status === "uncertain" ||
      agent.status === "tracking-error";
    if (filters.activeOnly && !active) continue;
    if (filters.model !== "all" && agent.effective_model !== filters.model) continue;
    if (filters.project !== "all" && agent.project !== filters.project) continue;
    if (query && !haystack.includes(query)) continue;

    selected.add(agent.thread_id);
    let parentId = agent.parent_thread_id;
    const seen = new Set<string>();
    while (parentId && !seen.has(parentId)) {
      seen.add(parentId);
      selected.add(parentId);
      parentId = byId.get(parentId)?.parent_thread_id ?? null;
    }
  }

  return agents.filter((agent) => selected.has(agent.thread_id));
}

function formatAge(milliseconds: number | null) {
  if (milliseconds === null) return null;
  if (milliseconds < 5_000) return "刚刚更新";
  const seconds = Math.floor(milliseconds / 1_000);
  if (seconds < 60) return `${seconds} 秒前`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  return `${Math.floor(hours / 24)} 天前`;
}

function formatFullTime(timestamp: number | null) {
  if (timestamp === null || !Number.isFinite(timestamp) || timestamp < 0) return "时间待确认";
  return new Date(timestamp).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "medium",
  });
}
