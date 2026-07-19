import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ForceRestartImpact,
  RoutingOperationReceipt,
  RoutingUiSnapshot,
} from "../../shared/routing-types";
import { ROUTING_REFRESH_MS } from "../config";
import { routingApi } from "../lib/routingApi";

const MALFORMED_MESSAGE = "路由状态异常，已保留上一次验证通过的快照并进入降级模式。";
const RESTART_FAILURE_MESSAGES: Record<string, string> = {
  "confirmation-expired": "确认票据已过期，请重新检查影响并再次确认。",
  "impact-changed": "运行中的子代理或进程树已经变化，必须重新确认。",
  "operation-conflict": "另一个生命周期操作正在进行，请等待其完成。",
  "identity-changed": "Codex 进程身份或创建时间已变化，操作已关闭失败。",
  "termination-failed": "部分进程无法终止；不会启动替代窗口或自动重试。",
  "old-tree-still-running": "旧 Codex 进程树仍未完全退出，因此没有启动新实例。",
  "terminal-partial-failure": "终止已开始但新会话未验证成功；不会自动循环重试。",
  "cdp-verification-failed": "新 Codex 的回环端口或浏览器身份验证失败。",
};

export function useRouting() {
  const [snapshot, setSnapshot] = useState<RoutingUiSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [degraded, setDegraded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [operation, setOperation] = useState<
    "install" | "restore" | "restart" | "preflight" | "toggle" | null
  >(null);
  const [receipt, setReceipt] = useState<RoutingOperationReceipt | null>(null);
  const [pendingForce, setPendingForce] = useState<ForceRestartImpact | null>(null);
  const polling = useRef(false);
  const operationActive = useRef(false);

  const accept = useCallback((next: RoutingUiSnapshot | null) => {
    if (next === null) {
      setDegraded(true);
      setError(MALFORMED_MESSAGE);
      return false;
    }
    setSnapshot(next);
    setDegraded(false);
    setError(null);
    return true;
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      accept(await routingApi.getSnapshot());
    } catch {
      setDegraded(true);
      setError("路由状态刷新失败，已保留上一次可用快照。功能会自动重试。");
    } finally {
      setRefreshing(false);
    }
  }, [accept]);

  const install = useCallback(async () => {
    if (operationActive.current) return null;
    operationActive.current = true;
    setOperation("install");
    try {
      const nextReceipt = await routingApi.install();
      setReceipt(nextReceipt);
      accept(await routingApi.getSnapshot());
      return nextReceipt;
    } catch {
      setDegraded(true);
      setError("Smart Routing 安装失败；未确认的结果不会应用到界面。");
      return null;
    } finally {
      operationActive.current = false;
      setOperation(null);
    }
  }, [accept]);

  const restore = useCallback(async () => {
    if (operationActive.current) return null;
    operationActive.current = true;
    setOperation("restore");
    try {
      const nextReceipt = await routingApi.restore();
      setReceipt(nextReceipt);
      accept(await routingApi.getSnapshot());
      return nextReceipt;
    } catch {
      setDegraded(true);
      setError("Smart Routing 恢复失败；存在冲突的用户配置不会被覆盖。");
      return null;
    } finally {
      operationActive.current = false;
      setOperation(null);
    }
  }, [accept]);

  const requestRestart = useCallback(async () => {
    if (operationActive.current) return null;
    operationActive.current = true;
    setOperation("restart");
    try {
      const nextReceipt = await routingApi.requestCodexRestart();
      setReceipt(nextReceipt);
      if (nextReceipt.status === "blocked" && nextReceipt.reason_codes.includes("active-child")) {
        setPendingForce(await routingApi.prepareForceRestart("routing-restart"));
      }
      accept(await routingApi.getSnapshot());
      return nextReceipt;
    } catch {
      setDegraded(true);
      setError("Codex 重启请求失败；当前 Codex 进程未被更改。");
      return null;
    } finally {
      operationActive.current = false;
      setOperation(null);
    }
  }, [accept]);

  const cancelForceRestart = useCallback(() => {
    if (pendingForce && operationActive.current) {
      void routingApi.cancelForceRestart(pendingForce.confirmation_ticket);
    }
    setPendingForce(null);
  }, [pendingForce]);

  const confirmForceRestart = useCallback(async () => {
    if (!pendingForce || operationActive.current) return null;
    operationActive.current = true;
    setOperation("restart");
    try {
      const nextReceipt = await routingApi.requestCodexRestart(
        "force-after-grace",
        pendingForce.confirmation_ticket,
      );
      setReceipt(nextReceipt);
      setPendingForce(null);
      accept(await routingApi.getSnapshot());
      if (nextReceipt.status !== "applied" && nextReceipt.status !== "noop") {
        setError(
          RESTART_FAILURE_MESSAGES[nextReceipt.reason_codes[0] ?? ""] ??
            "强制重启未完成；不会自动循环重试，请重新检查影响后再确认。",
        );
      }
      return nextReceipt;
    } catch {
      setError("强制重启失败；不会自动循环重试，也不会错误显示为已恢复。");
      return null;
    } finally {
      operationActive.current = false;
      setOperation(null);
    }
  }, [accept, pendingForce]);

  const beginPreflight = useCallback(
    async (rootConversationId: string) => {
      if (operationActive.current) return null;
      operationActive.current = true;
      setOperation("preflight");
      try {
        const nextReceipt = await routingApi.beginPreflight(rootConversationId);
        setReceipt(nextReceipt);
        accept(await routingApi.getSnapshot());
        return nextReceipt;
      } catch {
        setDegraded(true);
        setError("原生能力预检启动失败；未验证模型仍保持不可用。");
        return null;
      } finally {
        operationActive.current = false;
        setOperation(null);
      }
    },
    [accept],
  );

  const setRootEnabled = useCallback(
    async (rootConversationId: string, enabled: boolean) => {
      if (operationActive.current) return null;
      operationActive.current = true;
      setOperation("toggle");
      try {
        const nextReceipt = await routingApi.setRootEnabled(rootConversationId, enabled);
        setReceipt(nextReceipt);
        accept(await routingApi.getSnapshot());
        return nextReceipt;
      } catch {
        setDegraded(true);
        setError("根任务路由状态修改失败；已保留上一次验证状态。");
        return null;
      } finally {
        operationActive.current = false;
        setOperation(null);
      }
    },
    [accept],
  );

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | undefined;

    Promise.all([routingApi.getSnapshot(), routingApi.subscribe((next) => mounted && accept(next))])
      .then(([initial, stop]) => {
        if (!mounted) {
          stop();
          return;
        }
        accept(initial);
        unsubscribe = stop;
      })
      .catch(() => {
        if (mounted) {
          setDegraded(true);
          setError("无法连接 Smart Routing 本地服务，功能会自动重试。");
        }
      })
      .finally(() => mounted && setLoading(false));

    const poll = window.setInterval(() => {
      if (!mounted || polling.current) return;
      polling.current = true;
      routingApi
        .getSnapshot()
        .then((next) => mounted && accept(next))
        .catch(() => {
          if (mounted) {
            setDegraded(true);
            setError("路由状态轮询失败，已保留上一次可用快照。");
          }
        })
        .finally(() => {
          polling.current = false;
        });
    }, ROUTING_REFRESH_MS);

    return () => {
      mounted = false;
      window.clearInterval(poll);
      unsubscribe?.();
    };
  }, [accept]);

  return {
    snapshot,
    loading,
    refreshing,
    degraded,
    error,
    connected: snapshot !== null && !degraded,
    operation,
    receipt,
    pendingForce,
    refresh,
    install,
    restore,
    requestRestart,
    confirmForceRestart,
    cancelForceRestart,
    beginPreflight,
    setRootEnabled,
  };
}
