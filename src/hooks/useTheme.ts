import { useCallback, useEffect, useRef, useState } from "react";
import type { ThemeOperationReceipt, ThemeUiSnapshot } from "../../shared/theme-types";
import { themeApi } from "../lib/themeApi";

type ThemeOperation = "start-session" | "apply" | "restore";
const REFRESH_MS = 5_000;

export function useTheme() {
  const [snapshot, setSnapshot] = useState<ThemeUiSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [degraded, setDegraded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [operation, setOperation] = useState<ThemeOperation | null>(null);
  const [receipt, setReceipt] = useState<ThemeOperationReceipt | null>(null);
  const operationActive = useRef(false);
  const polling = useRef(false);

  const accept = useCallback((next: ThemeUiSnapshot | null) => {
    if (next === null) {
      setDegraded(true);
      setError("主题状态异常，已保留上一次验证通过的快照。");
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
      accept(await themeApi.getSnapshot());
    } catch {
      setDegraded(true);
      setError("主题状态刷新失败，已保留上一次可用快照。");
    } finally {
      setRefreshing(false);
    }
  }, [accept]);

  const mutate = useCallback(
    async (kind: ThemeOperation, action: () => Promise<ThemeOperationReceipt>) => {
      if (operationActive.current) return null;
      operationActive.current = true;
      setOperation(kind);
      try {
        const nextReceipt = await action();
        setReceipt(nextReceipt);
        accept(await themeApi.getSnapshot());
        return nextReceipt;
      } catch {
        setDegraded(true);
        setError(
          kind === "start-session"
            ? "主题会话启动失败；Codex 未确认进入可换肤状态。"
            : "主题操作失败；未确认的外观不会显示为已应用。",
        );
        return null;
      } finally {
        operationActive.current = false;
        setOperation(null);
      }
    },
    [accept],
  );

  const startSession = useCallback(
    () => mutate("start-session", () => themeApi.startSession()),
    [mutate],
  );
  const apply = useCallback(
    (themeId: string) => mutate("apply", () => themeApi.apply(themeId)),
    [mutate],
  );
  const restore = useCallback(() => mutate("restore", () => themeApi.restore()), [mutate]);

  useEffect(() => {
    let mounted = true;
    themeApi
      .getSnapshot()
      .then((next) => mounted && accept(next))
      .catch(() => {
        if (mounted) {
          setDegraded(true);
          setError("无法连接本地主题服务，功能会自动重试。");
        }
      })
      .finally(() => mounted && setLoading(false));

    const interval = window.setInterval(() => {
      if (!mounted || polling.current || operationActive.current) return;
      polling.current = true;
      themeApi
        .getSnapshot()
        .then((next) => mounted && accept(next))
        .catch(() => {
          if (mounted) {
            setDegraded(true);
            setError("主题状态轮询失败，已保留上一次可用快照。");
          }
        })
        .finally(() => {
          polling.current = false;
        });
    }, REFRESH_MS);

    return () => {
      mounted = false;
      window.clearInterval(interval);
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
    startSession,
    apply,
    restore,
  };
}
