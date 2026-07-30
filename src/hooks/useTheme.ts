import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ForceRestartImpact,
  ThemeEnvironmentReport,
  ThemeOperationReceipt,
  ThemeRestartIntent,
  ThemeUiSnapshot,
} from "../../shared/theme-types";
import { themeApi } from "../lib/themeApi";

type ThemeOperation = "start-session" | "activate" | "import" | "restore";
const REFRESH_MS = 5_000;
const OPERATION_TIMEOUT_MS = 120_000;
const FAILURE_MESSAGES: Record<string, string> = {
  "monitor-uncertain": "代理监控数据不完整；为避免打断任务，普通重启已阻止。",
  "confirmation-expired": "确认票据已过期，请重新检查影响并再次确认。",
  "impact-changed": "运行中的任务或进程树已经变化，必须重新确认。",
  "operation-conflict": "另一个生命周期操作正在进行，请等待其完成。",
  "identity-changed": "Codex 进程身份或创建时间已变化，操作已关闭失败。",
  "termination-failed": "部分进程无法终止；不会启动替代窗口或自动重试。",
  "old-tree-still-running": "旧 Codex 进程树仍未完全退出，因此没有启动新实例。",
  "terminal-partial-failure": "终止已开始但新会话未验证成功；不会自动循环重试。",
  "cdp-verification-failed": "新 Codex 的回环端口或浏览器身份验证失败。",
  "cdp-unavailable": "当前 Codex 控制会话不可用；请恢复主题会话后重试。",
  "dom-incompatible": "当前 Codex 页面结构与该主题不兼容，已保留原外观。",
  "multiple-windows": "检测到多个可见的 Codex 主页面，请只保留一个窗口后重试。",
  "partial-apply-failed": "并非所有 Codex 页面都兼容，主题未标记为已应用。",
};

class ThemeOperationTimeoutError extends Error {}

function withOperationTimeout<T>(operation: Promise<T>): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timeoutId = window.setTimeout(
      () => reject(new ThemeOperationTimeoutError("Theme operation timed out")),
      OPERATION_TIMEOUT_MS,
    );
    operation.then(
      (value) => {
        window.clearTimeout(timeoutId);
        resolve(value);
      },
      (error: unknown) => {
        window.clearTimeout(timeoutId);
        reject(error);
      },
    );
  });
}

function failureMessage(receipt: ThemeOperationReceipt): string {
  return (
    FAILURE_MESSAGES[receipt.reason_codes[0] ?? ""] ??
    "主题操作未完成；请根据失败原因重试，已应用主题不会被误报。"
  );
}

export function useTheme() {
  const [snapshot, setSnapshot] = useState<ThemeUiSnapshot | null>(null);
  const [environment, setEnvironment] = useState<ThemeEnvironmentReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [degraded, setDegraded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [operation, setOperation] = useState<ThemeOperation | null>(null);
  const [operationThemeId, setOperationThemeId] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<ThemeOperationReceipt | null>(null);
  const [pendingForce, setPendingForce] = useState<ForceRestartImpact | null>(null);
  const [pendingThemeId, setPendingThemeId] = useState<string | null>(null);
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
    return true;
  }, []);

  const acceptEnvironment = useCallback((next: ThemeEnvironmentReport | null) => {
    if (next === null) {
      setDegraded(true);
      setError("本机环境检测返回了无效结果，请重新安装最新版 Codex Assistant。");
      return false;
    }
    setEnvironment(next);
    return true;
  }, []);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const [nextSnapshot, nextEnvironment] = await Promise.all([
        themeApi.getSnapshot(),
        themeApi.getEnvironment(),
      ]);
      if (accept(nextSnapshot) && acceptEnvironment(nextEnvironment)) setError(null);
    } catch {
      setDegraded(true);
      setError("主题状态刷新失败，已保留上一次可用快照。");
    } finally {
      setRefreshing(false);
    }
  }, [accept, acceptEnvironment]);

  const mutate = useCallback(
    async (
      kind: ThemeOperation,
      action: () => Promise<ThemeOperationReceipt>,
      forceIntent?: ThemeRestartIntent,
      themeId?: string,
    ) => {
      if (operationActive.current) return null;
      operationActive.current = true;
      setOperation(kind);
      setOperationThemeId(kind === "activate" ? (themeId ?? null) : null);
      setError(null);
      try {
        const nextReceipt = await withOperationTimeout(action());
        setReceipt(nextReceipt);
        if (
          nextReceipt.status === "blocked" &&
          (nextReceipt.reason_codes.includes("active-work") ||
            nextReceipt.reason_codes.includes("monitor-uncertain")) &&
          forceIntent
        ) {
          try {
            const impact = await themeApi.prepareForceRestart(forceIntent, themeId);
            setPendingForce(impact);
            setPendingThemeId(themeId ?? null);
          } catch {
            setPendingForce(null);
            setPendingThemeId(null);
            setError(failureMessage(nextReceipt));
          }
        }
        const [nextSnapshot, nextEnvironment] = await Promise.all([
          themeApi.getSnapshot(),
          themeApi.getEnvironment(),
        ]);
        accept(nextSnapshot);
        acceptEnvironment(nextEnvironment);
        if (
          nextReceipt.status === "failed" ||
          (nextReceipt.status === "blocked" &&
            !nextReceipt.reason_codes.includes("active-work") &&
            !nextReceipt.reason_codes.includes("monitor-uncertain"))
        ) {
          setError(failureMessage(nextReceipt));
        }
        return nextReceipt;
      } catch (caught) {
        if (caught instanceof ThemeOperationTimeoutError) {
          setError("主题操作超时，已解除界面等待；请刷新状态后重新尝试。");
        } else {
          setDegraded(true);
          setError(
            kind === "start-session"
              ? "主题会话启动失败；Codex 未确认进入可换肤状态。"
              : "主题操作失败；未确认的外观不会显示为已应用。",
          );
        }
        return null;
      } finally {
        operationActive.current = false;
        setOperation(null);
        setOperationThemeId(null);
      }
    },
    [accept, acceptEnvironment],
  );

  const startSession = useCallback(
    () => mutate("start-session", () => themeApi.startSession(), "theme-session"),
    [mutate],
  );
  const activate = useCallback(
    (themeId: string) =>
      mutate("activate", () => themeApi.activate(themeId), "activate-theme", themeId),
    [mutate],
  );
  const importLocalImage = useCallback(
    async (name: string, imageDataUrl: string) => {
      if (operationActive.current) return null;
      operationActive.current = true;
      setOperation("import");
      setError(null);
      try {
        const imported = await themeApi.importLocalImage(name, imageDataUrl);
        accept(await themeApi.getSnapshot());
        acceptEnvironment(await themeApi.getEnvironment());
        setOperationThemeId(imported.theme_id);
        const nextReceipt = await withOperationTimeout(themeApi.activate(imported.theme_id));
        setReceipt(nextReceipt);
        const [nextSnapshot, nextEnvironment] = await Promise.all([
          themeApi.getSnapshot(),
          themeApi.getEnvironment(),
        ]);
        accept(nextSnapshot);
        acceptEnvironment(nextEnvironment);
        if (nextReceipt.status !== "applied" && nextReceipt.status !== "noop") {
          setError(failureMessage(nextReceipt));
        }
        return nextReceipt;
      } catch {
        setError("本机图片导入失败；仅支持不超过 1.45 MB 的 JPEG、PNG 或 WebP 文件。");
        return null;
      } finally {
        operationActive.current = false;
        setOperation(null);
        setOperationThemeId(null);
      }
    },
    [accept, acceptEnvironment],
  );
  const restore = useCallback(() => mutate("restore", () => themeApi.restore()), [mutate]);

  const cancelForceRestart = useCallback(() => {
    if (pendingForce && operationActive.current) {
      void themeApi.cancelForceRestart(pendingForce.confirmation_ticket);
    }
    setPendingForce(null);
    setPendingThemeId(null);
  }, [pendingForce]);

  const confirmForceRestart = useCallback(async () => {
    if (!pendingForce || operationActive.current) return null;
    operationActive.current = true;
    const kind = pendingForce.intent === "theme-session" ? "start-session" : "activate";
    setOperation(kind);
    setOperationThemeId(kind === "activate" ? pendingThemeId : null);
    setError(null);
    try {
      const nextReceipt = await withOperationTimeout(
        pendingForce.intent === "activate-theme" && pendingThemeId
          ? themeApi.activate(pendingThemeId, "force-after-grace", pendingForce.confirmation_ticket)
          : themeApi.startSession("force-after-grace", pendingForce.confirmation_ticket),
      );
      setReceipt(nextReceipt);
      setPendingForce(null);
      setPendingThemeId(null);
      const [nextSnapshot, nextEnvironment] = await Promise.all([
        themeApi.getSnapshot(),
        themeApi.getEnvironment(),
      ]);
      accept(nextSnapshot);
      acceptEnvironment(nextEnvironment);
      if (nextReceipt.status !== "applied" && nextReceipt.status !== "noop") {
        setError(failureMessage(nextReceipt));
      }
      return nextReceipt;
    } catch {
      setError("强制重启失败；不会自动循环重试，也不会显示为已恢复。");
      return null;
    } finally {
      operationActive.current = false;
      setOperation(null);
      setOperationThemeId(null);
    }
  }, [accept, acceptEnvironment, pendingForce, pendingThemeId]);

  useEffect(() => {
    let mounted = true;
    Promise.all([themeApi.getSnapshot(), themeApi.getEnvironment()])
      .then(
        ([nextSnapshot, nextEnvironment]) =>
          mounted && accept(nextSnapshot) && acceptEnvironment(nextEnvironment),
      )
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
      Promise.all([themeApi.getSnapshot(), themeApi.getEnvironment()])
        .then(([nextSnapshot, nextEnvironment]) => {
          if (mounted) {
            accept(nextSnapshot);
            acceptEnvironment(nextEnvironment);
          }
        })
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
  }, [accept, acceptEnvironment]);

  return {
    snapshot,
    environment,
    loading,
    refreshing,
    degraded,
    error,
    connected: snapshot !== null && !degraded,
    operation,
    operationThemeId,
    receipt,
    pendingForce,
    refresh,
    refreshEnvironment: refresh,
    startSession,
    activate,
    importLocalImage,
    restore,
    confirmForceRestart,
    cancelForceRestart,
  };
}
