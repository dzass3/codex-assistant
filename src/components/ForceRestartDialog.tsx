import { useEffect, useRef, useState } from "react";
import type { ForceRestartImpact } from "../../shared/theme-types";

export function ForceRestartDialog({
  impact,
  busy,
  onCancel,
  onConfirm,
  returnFocus,
}: {
  impact: ForceRestartImpact;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  returnFocus?: HTMLElement | null;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const [remainingSeconds, setRemainingSeconds] = useState(5);

  useEffect(() => {
    if (!busy) {
      setRemainingSeconds(5);
      return;
    }
    const timer = window.setInterval(
      () => setRemainingSeconds((current) => Math.max(0, current - 1)),
      1_000,
    );
    return () => window.clearInterval(timer);
  }, [busy]);

  useEffect(() => {
    returnFocusRef.current =
      returnFocus?.isConnected === true
        ? returnFocus
        : document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
    cancelRef.current?.focus();
    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancel();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = Array.from(
        dialogRef.current?.querySelectorAll<HTMLElement>("button:not(:disabled)") ?? [],
      );
      if (focusable.length === 0) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("keydown", handleKey);
      if (returnFocusRef.current?.isConnected) returnFocusRef.current.focus();
    };
  }, [onCancel, returnFocus]);

  return (
    <div className="dialog-backdrop force-restart-backdrop">
      <div
        ref={dialogRef}
        className="settings-dialog force-restart-dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="force-restart-title"
        aria-describedby="force-restart-description"
      >
        <span className="eyebrow">DESTRUCTIVE EXCEPTION</span>
        <h2 id="force-restart-title">
          {impact.monitor_confident ? "结束运行中的任务并重启？" : "监控状态不确定，仍要重启？"}
        </h2>
        <p id="force-restart-description" className="dialog-copy">
          {impact.monitor_confident
            ? `当前有 ${impact.active_work_count} 个 Codex 任务仍在运行。`
            : "当前无法可靠确认是否仍有任务运行。"}
          确认后会等待 5 秒，再结束已验证的 Codex 进程树；未完成工作可能丢失。
        </p>
        <div className="force-restart-warning" role="alert">
          票据仅在 60 秒内有效，进程、身份或影响数量变化后必须重新确认。终止开始后不会自动循环重试。
        </div>
        <div className="operation-stage" aria-live="polite" aria-atomic="true">
          {busy
            ? remainingSeconds > 0
              ? `正在等待任务停止，强制终止前还会等待 ${remainingSeconds} 秒…`
              : "正在重新验证进程树；操作可能已进入不可逆阶段…"
            : "等待你的明确确认"}
        </div>
        <div className="dialog-actions">
          <button ref={cancelRef} className="button-secondary" onClick={onCancel}>
            {busy ? "取消等待" : "取消"}
          </button>
          <button className="button-danger" onClick={onConfirm} disabled={busy}>
            {busy ? "正在执行…" : "结束任务并强制重启"}
          </button>
        </div>
      </div>
    </div>
  );
}
