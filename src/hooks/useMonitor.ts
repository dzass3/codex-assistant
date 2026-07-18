import { useCallback, useEffect, useState } from "react";
import type { MonitorSettings, MonitorSnapshot } from "../../shared/monitor-types";
import { monitorApi } from "../lib/monitorApi";

export function useMonitor() {
  const [snapshot, setSnapshot] = useState<MonitorSnapshot | null>(null);
  const [settings, setSettings] = useState<MonitorSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | undefined;

    Promise.all([
      monitorApi.getSnapshot(),
      monitorApi.getSettings(),
      monitorApi.subscribe(setSnapshot),
    ])
      .then(([initial, currentSettings, stop]) => {
        if (!mounted) {
          stop();
          return;
        }
        setSnapshot(initial);
        setSettings(currentSettings);
        unsubscribe = stop;
        setError(null);
      })
      .catch(() => mounted && setError("无法连接本地监控服务，请重新打开应用。"))
      .finally(() => mounted && setLoading(false));

    return () => {
      mounted = false;
      unsubscribe?.();
    };
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      setSnapshot(await monitorApi.refresh());
      setError(null);
    } catch {
      setError("刷新失败，已保留上一次可用快照。功能会自动重试。");
    } finally {
      setRefreshing(false);
    }
  }, []);

  const setCodexHome = useCallback(async (path: string) => {
    const next = await monitorApi.setCodexHome(path);
    setSettings(next);
    setSnapshot(await monitorApi.refresh());
  }, []);

  return {
    snapshot,
    settings,
    loading,
    refreshing,
    error,
    connected: snapshot !== null && !error,
    refresh,
    setCodexHome,
  };
}
