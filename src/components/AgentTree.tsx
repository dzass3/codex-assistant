import { useMemo, type CSSProperties } from "react";
import type { AgentObservation, AgentStatus } from "../../shared/monitor-types";
import type { MonitorFilters } from "./FilterBar";

const STATUS_LABELS: Record<AgentStatus, string> = {
  starting: "启动中",
  running: "运行中",
  idle: "可继续调用",
  interrupted: "已中断",
  "tracking-error": "跟踪异常",
};

export function AgentTree({
  agents,
  filters,
}: {
  agents: AgentObservation[];
  filters: MonitorFilters;
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
        <span className="empty-glyph">◎</span>
        <h3>{agents.length === 0 ? "暂未发现 Codex 任务" : "没有匹配的代理"}</h3>
        <p>
          {agents.length === 0
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
      {descendants.length > 0 && (
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
      )}
    </div>
  );
}

function AgentRow({ agent, childCount }: { agent: AgentObservation; childCount: number }) {
  const age = formatAge(agent.freshness_ms);
  const source = {
    "turn-context": "运行确认",
    "state-database": "状态库",
    "requested-only": "仅请求值",
    unknown: "未知来源",
  }[agent.model_source];

  return (
    <article
      className={`agent-row agent-row--${agent.status}`}
      data-testid={`agent-${agent.thread_id}`}
    >
      <div className="agent-rail">
        <span className={`status-light status-light--${agent.status}`} />
      </div>
      <div className="agent-identity">
        <div className="agent-name-line">
          <strong>{agent.display_name}</strong>
          {agent.is_subagent && <span className="subagent-tag">子代理</span>}
          {childCount > 0 && <span className="child-count">{childCount} 个下级</span>}
        </div>
        <p>{[agent.role, agent.project, age].filter(Boolean).join(" · ") || "任务元数据待更新"}</p>
      </div>
      <div className="model-column">
        <div className="model-line">
          <span className="model-badge">{agent.effective_model ?? "尚未确认"}</span>
          {agent.reasoning_effort && <span className="effort-badge">{agent.reasoning_effort}</span>}
        </div>
        <small>{source}</small>
      </div>
      {agent.model_drift ? (
        <div
          className="drift-badge"
          title={`请求 ${agent.requested_model ?? "未知"}，实际 ${agent.effective_model ?? "未知"}`}
        >
          <span>模型漂移</span>
          <small>
            {agent.requested_model} → {agent.effective_model}
          </small>
        </div>
      ) : (
        <span className="no-drift">模型一致</span>
      )}
      <span className={`status-pill status-pill--${agent.status}`}>
        {STATUS_LABELS[agent.status]}
      </span>
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
  return `${Math.floor(minutes / 60)} 小时前`;
}
