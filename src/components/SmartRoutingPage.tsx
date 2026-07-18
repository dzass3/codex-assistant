import { useEffect, useState } from "react";
import { useRouting } from "../hooks/useRouting";

const CHANGE_LABELS: Record<string, string> = {
  "agents.max_depth": "允许两层原生子代理",
  "agents.codex_assistant_spark": "GPT-5.3 Codex Spark 代理配置",
  "agents.codex_assistant_luna": "GPT-5.6 Luna 代理配置",
  "agents.codex_assistant_terra": "GPT-5.6 Terra 代理配置",
  "agents.codex_assistant_sol": "GPT-5.6 Sol 代理配置",
  "mcp_servers.codex_assistant_routing": "元数据路由 MCP",
  "skill.codex-assistant-routing": "对话路由技能",
};

const TIER_LABELS = {
  spark: "Spark",
  luna: "Luna",
  terra: "Terra",
  sol: "Sol",
};
const ELIGIBILITY_LABELS = {
  unknown: "待验证",
  verifying: "验证中",
  eligible: "已验证",
  unavailable: "不可用",
  stale: "需重新验证",
};
const REASON_LABELS: Record<string, string> = {
  "awaiting-visible-command": "等待可见验证指令",
  "awaiting-native-child": "等待原生子代理",
  "awaiting-effective-model": "等待有效模型元数据",
  "child-still-running": "验证子代理仍在运行",
  "effective-model-mismatch": "有效模型与请求不一致",
  "native-profile-rejected": "原生配置被 Codex 拒绝",
  "lineage-ambiguous": "原生代理层级不明确",
  "detached-process": "检测到非原生独立进程",
  "unrelated-root": "子代理不属于当前根任务",
  "missing-parent": "缺少原生父代理",
  "parent-not-verified-terra": "需要已验证的 Terra 父代理",
  timeout: "原生验证超时",
  "host-version-changed": "Codex 版本已变化",
  "profile-version-changed": "路由配置版本已变化",
};

export interface RoutingRootOption {
  conversationId: string;
  label: string;
}

const EMPTY_ROOTS: RoutingRootOption[] = [];

export function SmartRoutingPage({ roots = EMPTY_ROOTS }: { roots?: RoutingRootOption[] }) {
  const routing = useRouting();
  const setup = routing.snapshot?.setup;
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [selectedRoot, setSelectedRoot] = useState(roots[0]?.conversationId ?? "");

  useEffect(() => {
    if (!roots.some((root) => root.conversationId === selectedRoot)) {
      setSelectedRoot(roots[0]?.conversationId ?? "");
    }
  }, [roots, selectedRoot]);

  if (routing.loading) {
    return (
      <section className="routing-page empty-state" aria-label="Smart Routing">
        <span className="loading-ring" />
        <h2>正在检查 Smart Routing</h2>
      </section>
    );
  }

  return (
    <section className="routing-page">
      <div className="routing-page__heading">
        <div>
          <span className="eyebrow">QUALITY-FIRST NATIVE DELEGATION</span>
          <h2>Smart Routing</h2>
          <p>按任务复杂度选择已验证的原生子代理模型，质量优先，额度与时间其次。</p>
        </div>
        <button className="icon-button" onClick={routing.refresh} disabled={routing.refreshing}>
          {routing.refreshing ? "刷新中" : "刷新状态"}
        </button>
      </div>

      {routing.error && <div className="global-error">{routing.error}</div>}

      <div className="routing-grid">
        <article className="routing-panel routing-panel--setup">
          <span className="eyebrow">SETUP</span>
          <h3>{setup?.installation_status === "uninstalled" ? "尚未安装" : "安装状态"}</h3>
          <p>安装仅写入 Codex Assistant 自有代理、MCP 与技能配置，并保留可恢复备份。</p>
          {setup?.config_changes.length ? (
            <div className="routing-change-list">
              <strong>预计配置变更</strong>
              <ul>
                {setup.config_changes.map((change) => (
                  <li key={change}>{CHANGE_LABELS[change]}</li>
                ))}
              </ul>
            </div>
          ) : null}
          {setup?.installation_status === "uninstalled" && (
            <button
              className="button-primary"
              onClick={() => void routing.install()}
              disabled={routing.operation !== null}
            >
              {routing.operation === "install" ? "正在安装…" : "安装 Smart Routing"}
            </button>
          )}
          {setup?.installation_status !== "uninstalled" && setup?.backup_label ? (
            <div className="routing-backup">
              <span>可恢复备份</span>
              <code>{setup.backup_label}</code>
              <button
                className="button-secondary"
                onClick={() => setRestoreOpen(true)}
                disabled={routing.operation !== null}
              >
                恢复官方配置
              </button>
            </div>
          ) : null}
          {setup && setup.restart_status !== "not-required" ? (
            <div className="routing-restart-notice" role="status">
              <strong>
                {setup.restart_status === "blocked-active-child"
                  ? "有原生子代理正在运行，重启已阻止"
                  : "需要让 Codex 载入新配置"}
              </strong>
              <p>仅在没有运行中的原生子代理时，才会安全关闭并重启官方 Codex 一次。</p>
              <button
                className="button-primary"
                onClick={() => void routing.requestRestart()}
                disabled={
                  setup.restart_status === "blocked-active-child" || routing.operation !== null
                }
              >
                {routing.operation === "restart" ? "正在检查并重启…" : "安全重启 Codex 一次"}
              </button>
            </div>
          ) : null}
        </article>

        <article className="routing-panel">
          <span className="eyebrow">SECURITY BOUNDARY</span>
          <h3>本机、同用户、元数据限定</h3>
          <p>
            CDP 端点仅绑定回环地址，并验证为同一 Windows 用户启动的官方 Codex 进程。 Codex Assistant
            不会读取或保存对话内容、推理文本、工具参数或输出。
          </p>
        </article>
      </div>

      {setup?.installation_status === "installed" && setup.restart_status === "not-required" ? (
        <article className="routing-panel routing-preflight">
          <div>
            <span className="eyebrow">NATIVE PREFLIGHT</span>
            <h3>
              {setup.preflight_status === "running"
                ? "正在验证原生模型能力"
                : setup.preflight_status === "complete"
                  ? "原生模型能力已验证"
                  : "开始原生模型能力验证"}
            </h3>
            <p>
              会在当前任务的原生子代理面板中验证请求模型、有效模型与真实父子层级；不会执行用户工作。
            </p>
          </div>
          <div className="routing-preflight__actions">
            <label>
              <span className="sr-only">选择根任务</span>
              <select
                aria-label="选择根任务"
                value={selectedRoot}
                onChange={(event) => setSelectedRoot(event.target.value)}
                disabled={setup.preflight_status === "running"}
              >
                {roots.length === 0 ? <option value="">没有可见根任务</option> : null}
                {roots.map((root) => (
                  <option key={root.conversationId} value={root.conversationId}>
                    {root.label}
                  </option>
                ))}
              </select>
            </label>
            <button
              className="button-primary"
              onClick={() => void routing.beginPreflight(selectedRoot)}
              disabled={
                !selectedRoot ||
                setup.cdp_status !== "ready" ||
                setup.preflight_status === "running" ||
                routing.operation !== null
              }
            >
              {routing.operation === "preflight" ? "正在启动预检…" : "开始原生能力预检"}
            </button>
          </div>
        </article>
      ) : null}

      {routing.snapshot?.routing.routes.length ? (
        <article className="routing-panel">
          <div className="routing-panel__heading">
            <div>
              <span className="eyebrow">CURRENT ROOTS</span>
              <h3>当前根任务路由</h3>
            </div>
            <span className="privacy-note">每个开关只绑定一个可见根任务</span>
          </div>
          <div className="routing-root-list">
            {routing.snapshot.routing.routes.map((route) => {
              const label =
                roots.find((root) => root.conversationId === route.conversation_id)?.label ??
                `任务 ${route.conversation_id.slice(0, 8)}`;
              const hasActiveChild = routing.snapshot?.routing.activity.some(
                (activity) =>
                  activity.route_key === route.route_key &&
                  ["classifying", "implementing", "reviewing"].includes(activity.phase),
              );
              return (
                <div className="routing-root-row" key={route.route_key}>
                  <div>
                    <strong>{label}</strong>
                    <span>{route.enabled ? `已启用 · ${route.phase}` : "已关闭"}</span>
                  </div>
                  <button
                    className={route.enabled ? "button-secondary" : "button-primary"}
                    aria-label={`${route.enabled ? "关闭" : "启用"} ${label} Smart Routing`}
                    onClick={() =>
                      void routing.setRootEnabled(route.conversation_id, !route.enabled)
                    }
                    disabled={routing.operation !== null || (route.enabled && hasActiveChild)}
                  >
                    {route.enabled && hasActiveChild
                      ? "等待子代理结束"
                      : route.enabled
                        ? "关闭"
                        : "启用"}
                  </button>
                </div>
              );
            })}
          </div>
        </article>
      ) : null}

      {routing.snapshot?.routing.eligibility.length ? (
        <article className="routing-panel routing-panel--wide">
          <div className="routing-panel__heading">
            <div>
              <span className="eyebrow">NATIVE CAPABILITY MATRIX</span>
              <h3>原生模型能力验证</h3>
            </div>
            <span className="privacy-note">请求模型不等于有效模型 · 仅已验证项可路由</span>
          </div>
          <div className="routing-table-wrap">
            <table className="routing-table">
              <thead>
                <tr>
                  <th>模型</th>
                  <th>路由</th>
                  <th>请求模型</th>
                  <th>有效模型</th>
                  <th>Codex 版本</th>
                  <th>状态</th>
                  <th>原因</th>
                </tr>
              </thead>
              <tbody>
                {routing.snapshot.routing.eligibility.map((entry) => (
                  <tr key={`${entry.tier}-${entry.route_kind}-${entry.depth}`}>
                    <th scope="row">{TIER_LABELS[entry.tier]}</th>
                    <td>{entry.route_kind === "direct" ? "直接" : "嵌套"}</td>
                    <td>{entry.requested_model}</td>
                    <td>{entry.status === "eligible" ? entry.requested_model : "未确认"}</td>
                    <td>{entry.codex_package_version}</td>
                    <td>{ELIGIBILITY_LABELS[entry.status]}</td>
                    <td>{entry.reason ? REASON_LABELS[entry.reason] : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </article>
      ) : null}

      {setup?.installation_status !== "uninstalled" ? (
        <div className="routing-policy-grid">
          <article className="routing-panel">
            <span className="eyebrow">QUALITY FLOOR</span>
            <h3>质量优先模型边界</h3>
            <ul className="routing-policy-list">
              <li>机械任务使用 Spark：规格完整、低风险、可机械验证</li>
              <li>低风险边界任务使用 Luna：范围清晰、改动局部、验收明确</li>
              <li>跨层与判断型任务使用 Terra：跨文件、跨层或需要独立判断</li>
              <li>架构与高风险任务使用 Sol：歧义、架构、安全及最终整体复审</li>
            </ul>
          </article>
          <article className="routing-panel">
            <span className="eyebrow">HARD BUDGETS</span>
            <h3>并发与质量预算</h3>
            <ul className="routing-policy-list">
              <li>每个根任务最多 3 个活跃子代理</li>
              <li>同一时刻最多 1 个嵌套子代理</li>
              <li>单个子任务最多 2 次质量升级与修复</li>
              <li>每次实现必须经过独立复审，失败则修复或升级模型</li>
            </ul>
          </article>
        </div>
      ) : null}

      {restoreOpen ? (
        <div className="dialog-backdrop">
          <div
            className="settings-dialog"
            role="dialog"
            aria-label="恢复官方配置"
            aria-modal="true"
          >
            <h2>恢复官方配置</h2>
            <p className="dialog-copy">
              将只恢复 Codex Assistant
              安装时记录的自有配置和文件。若检测到安装后的人为修改，恢复会停止并报告冲突。
            </p>
            <div className="dialog-actions">
              <button className="button-secondary" onClick={() => setRestoreOpen(false)}>
                取消
              </button>
              <button
                className="button-primary"
                onClick={() => {
                  setRestoreOpen(false);
                  void routing.restore();
                }}
              >
                确认恢复
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </section>
  );
}
