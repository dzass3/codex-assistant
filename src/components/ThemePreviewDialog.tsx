import { useEffect, useRef, useState, type CSSProperties } from "react";
import type { ThemePack } from "../../shared/theme-types";
import { themeApi } from "../lib/themeApi";

export function ThemePreviewDialog({
  pack,
  busy,
  onApply,
  onClose,
  returnFocus,
}: {
  pack: ThemePack;
  busy: boolean;
  onApply: () => void;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const closeRef = useRef<HTMLButtonElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);
  const [source, setSource] = useState<string | null>(
    pack.category === "local-import" ? null : pack.preview_path,
  );

  useEffect(() => {
    setSource(pack.category === "local-import" ? null : pack.preview_path);
    if (pack.category !== "local-import") return;
    let mounted = true;
    void themeApi
      .getPreviewDataUrl(pack.id)
      .then((value) => {
        if (mounted) setSource(value);
      })
      .catch(() => {
        if (mounted) setSource(null);
      });
    return () => {
      mounted = false;
    };
  }, [pack.category, pack.id, pack.preview_path]);

  useEffect(() => {
    returnFocusRef.current =
      returnFocus?.isConnected === true
        ? returnFocus
        : document.activeElement instanceof HTMLElement
          ? document.activeElement
          : null;
    closeRef.current?.focus();

    const handleKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
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
  }, [onClose, returnFocus]);

  const previewStyle = {
    "--preview-accent": pack.palette.accent,
    "--preview-border": pack.palette.border,
    "--preview-surface": pack.palette.surface,
    "--preview-surface-strong": pack.palette.surface_strong,
    "--preview-text": pack.palette.text,
    "--preview-image": source ? `url("${source}")` : "none",
    "--preview-position":
      pack.backdrop.kind === "image"
        ? `${pack.backdrop.focal_x}% ${pack.backdrop.focal_y}%`
        : "50% 50%",
  } as CSSProperties;

  return (
    <div
      className="theme-preview-backdrop"
      onMouseDown={(event) => {
        if (event.currentTarget === event.target) onClose();
      }}
    >
      <div
        ref={dialogRef}
        className="theme-preview-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="theme-preview-title"
        style={previewStyle}
      >
        <header className="theme-preview-dialog__header">
          <div>
            <span className="eyebrow">IMMERSIVE WORKSPACE PREVIEW</span>
            <h2 id="theme-preview-title">{pack.name} 主题实时预览</h2>
            <p>预览不会修改当前主题；只有点击“应用此主题”才会进入现有安全应用流程。</p>
          </div>
          <button
            ref={closeRef}
            type="button"
            className="theme-preview-dialog__close"
            aria-label="关闭主题预览"
            onClick={onClose}
          >
            <span aria-hidden="true">×</span>
          </button>
        </header>

        <div className="theme-preview-workspace" aria-label={`${pack.name} 模拟工作区`}>
          <aside className="theme-preview-workspace__sidebar">
            <strong>Sidebar</strong>
            <span className="theme-preview-brand">Codex</span>
            <nav aria-label="模拟导航">
              <span className="is-active">新建任务</span>
              <span>拉取请求</span>
              <span>站点</span>
              <span>插件</span>
            </nav>
            <div className="theme-preview-projects">
              <small>项目</small>
              <span>theme-gallery</span>
              <span>workspace-notes</span>
            </div>
          </aside>

          <main className="theme-preview-workspace__chat">
            <div className="theme-preview-workspace__section-label">Chat</div>
            <div className="theme-preview-message theme-preview-message--assistant">
              <small>Codex Assistant</small>
              <strong>让工作空间与灵感保持同一种氛围。</strong>
              <p>主题预览保留导航、消息、代码与输入区的完整层级。</p>
            </div>
            <div className="theme-preview-message theme-preview-message--user">
              继续完善主题中心。
            </div>
            <div className="theme-preview-workspace__input">
              <span>Input</span>
              <p>随心输入，让 Codex 陪你构建下一件作品…</p>
              <i aria-hidden="true">↑</i>
            </div>
          </main>

          <section className="theme-preview-workspace__terminal">
            <div className="theme-preview-workspace__section-label">Terminal</div>
            <code>
              <span>$ npm run check</span>
              <span className="is-success">✓ catalog contract</span>
              <span className="is-success">✓ accessible controls</span>
              <span className="is-accent">16 themes ready</span>
            </code>
          </section>
        </div>

        <footer className="theme-preview-dialog__footer">
          <div>
            <span
              className="theme-preview-dialog__swatch"
              style={{ background: pack.palette.accent }}
              aria-hidden="true"
            />
            <span>{pack.category === "local-import" ? "本机主题" : "离线官方主题"}</span>
          </div>
          <button
            type="button"
            className="button-primary"
            aria-label={`应用 ${pack.name}`}
            disabled={busy}
            onClick={onApply}
          >
            {busy ? "正在处理…" : "应用此主题"}
          </button>
        </footer>
      </div>
    </div>
  );
}
