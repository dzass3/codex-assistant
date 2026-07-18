import { useCallback, useEffect, useRef, useState } from "react";
import type { RoutingUiSnapshot } from "../../shared/routing-types";
import type { RoutingOperationReceipt } from "../../shared/routing-types";
import { ROUTING_REFRESH_MS } from "../config";
import { routingApi } from "../lib/routingApi";

const MALFORMED_MESSAGE = "路由状态异常，已保留上一次验证通过的快照并进入降级模式。";

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
    refresh,
    install,
    restore,
    requestRestart,
    beginPreflight,
    setRootEnabled,
  };
}
