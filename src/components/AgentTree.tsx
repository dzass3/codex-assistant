import { useMemo, useState, type CSSProperties } from "react";
import type { AgentObservation, AgentStatus } from "../../shared/monitor-types";
import type {
  RootRoutingControlSnapshot,
  RoutingActivationStatus,
} from "../../shared/routing-types";
import type { MonitorFilters } from "./FilterBar";

const STATUS_LABELS: Record<AgentStatus, string> = {
  starting: "启动中",
  running: "运行中",
  idle: "可继续调用",
  interrupted: "已中断",
  "tracking-error": "跟踪异常",
};

const ROUTING_STATUS_LABELS: Record<RoutingActivationStatus, string> = {
  off: "正常",
  "pending-open": "等待打开",
  "pending-next-turn": "下一条消息",
  enabled: "已启用",
  "needs-repair": "需要修复",
};

export interface AgentTreeRoutingControls {
  available: boolean;
  operationActive: boolean;
  routes: Array<{ conversation_id: string; enabled: boolean }>;
  controls: RootRoutingControlSnapshot[];
  onSetRootEnabled: (conversationId: string, enabled: boolean) => unknown;
}

export function AgentTree({
  agents,
  filters,
  routing,
}: {
  agents: AgentObservation[];
  filters: MonitorFilters;
  routing?: AgentTreeRoutingControls;
}) {
  const [explanationRoot, setExplanationRoot] = useState<string | null>(null);
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
        <AgentBranch
          key={agent.thread_id}
          agent={agent}
          childMap={childMap}
          ancestry={new Set()}
          routing={routing}
          explanationRoot={explanationRoot}
          onExplain={setExplanationRoot}
        />
      ))}
    </div>
  );
}

function AgentBranch({
  agent,
  childMap,
  ancestry,
  routing,
  explanationRoot,
  onExplain,
}: {
  agent: AgentObservation;
  childMap: Map<string | null, AgentObservation[]>;
  ancestry: Set<string>;
  routing?: AgentTreeRoutingControls;
  explanationRoot: string | null;
  onExplain: (conversationId: string | null) => void;
}) {
  if (ancestry.has(agent.thread_id)) return null;
  const nextAncestry = new Set(ancestry).add(agent.thread_id);
  const descendants = childMap.get(agent.thread_id) ?? [];
  return (
    <div className="agent-branch" style={{ "--depth": agent.depth } as CSSProperties}>
      <AgentRow
        agent={agent}
        childCount={descendants.length}
        routing={agent.is_subagent ? undefined : routing}
        explanationOpen={explanationRoot === agent.thread_id}
        onExplain={onExplain}
      />
      {descendants.length > 0 && (
        <div className="agent-children">
          {descendants.map((child) => (
            <AgentBranch
              key={child.thread_id}
              agent={child}
              childMap={childMap}
              ancestry={nextAncestry}
              routing={routing}
              explanationRoot={explanationRoot}
              onExplain={onExplain}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function AgentRow({
  agent,
  childCount,
  routing,
  explanationOpen,
  onExplain,
}: {
  agent: AgentObservation;
  childCount: number;
  routing?: AgentTreeRoutingControls;
  explanationOpen: boolean;
  onExplain: (conversationId: string | null) => void;
}) {
  const age = formatAge(agent.freshness_ms);
  const source = {
    "turn-context": "运行确认",
    "state-database": "状态库",
    "requested-only": "仅请求值",
    unknown: "未知来源",
  }[agent.model_source];
  const route = routing?.routes.find((entry) => entry.conversation_id === agent.thread_id);
  const enabled = route?.enabled ?? false;
  const control = routing?.controls.find((entry) => entry.conversation_id === agent.thread_id);
  const routingStatus = enabled ? (control?.status ?? "pending-open") : "off";

  return (
    <div className="agent-row-wrap">
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
          <p>
            {[agent.role, agent.project, age].filter(Boolean).join(" · ") || "任务元数据待更新"}
          </p>
        </div>
        <div className="model-column">
          <div className="model-line">
            <span className="model-badge">{agent.effective_model ?? "尚未确认"}</span>
            {agent.reasoning_effort && (
              <span className="effort-badge">{agent.reasoning_effort}</span>
            )}
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
        {routing ? (
          <div className="root-routing-control">
            <span className={`routing-state routing-state--${routingStatus}`}>
              {ROUTING_STATUS_LABELS[routingStatus]}
            </span>
            <button
              className={enabled ? "button-secondary" : "button-primary"}
              aria-label={`${enabled ? "关闭" : "启用"} ${agent.display_name} Smart Routing`}
              disabled={!routing.available || routing.operationActive}
              onClick={() => {
                routing.onSetRootEnabled(agent.thread_id, !enabled);
                if (!enabled) onExplain(agent.thread_id);
              }}
            >
              {enabled ? "关闭" : "Smart Routing"}
            </button>
            <details>
              <summary>效果与依据</summary>
              <p>质量优先按复杂度分配 Spark、Luna、Terra 或 Sol；关闭只影响后续任务。</p>
              <p>数据不足，暂不能估算本任务可节省的时间或额度。</p>
            </details>
          </div>
        ) : (
          <span className={`status-pill status-pill--${agent.status}`}>
            {STATUS_LABELS[agent.status]}
          </span>
        )}
      </article>
      {routing && explanationOpen ? (
        <div className="routing-first-use" role="status">
          <strong>质量优先 Smart Routing 已登记</strong>
          <p>打开该任务后会绑定到原生输入框；若当前已有一轮在运行，将从下一条消息开始生效。</p>
          <button className="button-secondary" onClick={() => onExplain(null)}>
            知道了
          </button>
        </div>
      ) : null}
    </div>
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
