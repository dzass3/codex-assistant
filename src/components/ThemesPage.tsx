import { useEffect, useRef, useState, type CSSProperties } from "react";
import type {
  ThemeCategory,
  ThemeEnvironmentCheckCode,
  ThemeEnvironmentReport,
} from "../../shared/theme-types";
import { useTheme } from "../hooks/useTheme";
import { themeApi } from "../lib/themeApi";
import { ForceRestartDialog } from "./ForceRestartDialog";

const CATEGORY_LABELS: Record<ThemeCategory, string> = {
  abstract: "原创抽象",
  "original-character": "原创角色",
  "project-showcase": "项目展示",
  "local-import": "仅限本地导入",
};

function ThemePreview({ themeId, name }: { themeId: string; name: string }) {
  const [source, setSource] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    themeApi
      .getPreviewDataUrl(themeId)
      .then((value) => {
        if (mounted) setSource(value);
      })
      .catch(() => {
        if (mounted) setSource(null);
      });
    return () => {
      mounted = false;
    };
  }, [themeId]);

  return source ? <CompleteThemePreview source={source} name={name} /> : null;
}

function CompleteThemePreview({ source, name }: { source: string; name: string }) {
  return (
    <>
      <img className="theme-card__preview-backdrop" src={source} alt="" aria-hidden="true" />
      <img className="theme-card__preview-artwork" src={source} alt={`${name} 主题预览`} />
    </>
  );
}

export function ThemesPage() {
  const themes = useTheme();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const forceRestartTriggerRef = useRef<HTMLElement | null>(null);
  const [importError, setImportError] = useState<string | null>(null);
  const snapshot = themes.snapshot;
  const ready = snapshot?.session_status === "ready";
  const paused = snapshot?.session_status === "paused";
  const selectedThemeId = snapshot?.selected_theme_id ?? null;
  const hasThemeState = Boolean(selectedThemeId || snapshot?.applied_theme_id);
  const environment = themes.environment;
  const nextAction = environment?.next_action;
  const sessionActionLabel =
    nextAction === "confirm-restart"
      ? "确认重启并应用"
      : nextAction === "launch-codex-for-theme"
        ? "启动并应用主题"
        : paused
          ? "恢复主题会话"
          : "启动主题会话";

  if (themes.loading) {
    return (
      <section className="themes-page empty-state" aria-label="主题管理">
        <span className="loading-ring" />
        <h2>正在检查主题引擎</h2>
      </section>
    );
  }

  return (
    <section className="themes-page">
      <div className="themes-page__heading">
        <div>
          <span className="eyebrow">SAFE · LOCAL · ONE-CLICK</span>
          <h2>一键换肤</h2>
          <p>
            选择内置主题，或导入一张你有权使用的本机图片；主题只装饰界面，不遮挡文字、图标与操作。
          </p>
        </div>
        <div className="themes-page__actions">
          <input
            ref={fileInputRef}
            className="sr-only"
            type="file"
            accept="image/jpeg,image/png,image/webp"
            tabIndex={-1}
            aria-hidden="true"
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              event.currentTarget.value = "";
              if (!file) return;
              setImportError(null);
              void readLocalThemeFile(file)
                .then((imageDataUrl) =>
                  themes.importLocalImage(file.name.replace(/\.[^.]+$/, ""), imageDataUrl),
                )
                .catch(() => setImportError("请选择不超过 1.45 MB 的 JPEG、PNG 或 WebP 图片。"));
            }}
          />
          <button
            className="button-primary"
            type="button"
            onClick={() => fileInputRef.current?.click()}
            disabled={themes.operation !== null}
          >
            {themes.operation === "import" ? "正在导入并应用…" : "导入本机图片"}
          </button>
          <button className="icon-button" onClick={themes.refresh} disabled={themes.refreshing}>
            {themes.refreshing ? "刷新中" : "刷新状态"}
          </button>
        </div>
      </div>

      {importError ? (
        <div className="global-error" role="alert">
          {importError}
        </div>
      ) : null}

      {themes.error ? (
        <div className="global-error" role="alert">
          {themes.error}
        </div>
      ) : null}

      {snapshot?.catalog_notice ? (
        <div className="theme-catalog-notice" role="status">
          {snapshot.catalog_notice}
        </div>
      ) : null}

      {environment ? <ThemeEnvironmentPanel report={environment} /> : null}

      <article className="theme-session-panel">
        <div>
          <span className="eyebrow">VERIFIED LOCAL SESSION</span>
          <h3>{ready ? "主题会话已就绪" : paused ? "主题已暂停" : "启动主题会话"}</h3>
          <p>
            {nextAction === "confirm-restart"
              ? "当前官方 ChatGPT/Codex 没有经过验证的主题会话。只有你确认后，Codex Assistant 才会关闭并重启官方应用；它不会在后台自动重启。"
              : nextAction === "launch-codex-for-theme"
                ? selectedThemeId
                  ? "官方 ChatGPT/Codex 当前未运行。点击后会按你的这一次操作启动官方应用并应用已保存主题。"
                  : "官方 ChatGPT/Codex 当前未运行。先选择主题，再由你点击启动并应用。"
                : paused
                  ? "主题选择已保留，但当前没有经过验证的控制会话。重新应用时会再次验证界面可用性。"
                  : "首次应用可能需要你确认重启官方 ChatGPT/Codex，以建立仅绑定本机、当前 Windows 用户和官方进程的主题会话。"}
          </p>
        </div>
        {!ready ? (
          <button
            className="button-primary"
            onClick={(event) => {
              forceRestartTriggerRef.current = event.currentTarget;
              void (paused && selectedThemeId
                ? themes.activate(selectedThemeId)
                : themes.startSession());
            }}
            disabled={themes.operation !== null}
          >
            {themes.operation === "start-session" || themes.operation === "activate"
              ? "正在安全启动…"
              : sessionActionLabel}
          </button>
        ) : (
          <span className="theme-session-ready" role="status">
            本地会话已验证
          </span>
        )}
      </article>

      <div className="theme-rights-notice">
        <strong>分发边界</strong>
        <p>
          内置主题只包含项目原创且版权已核验的素材。名人、动漫/IP
          和第三方仓库截图不会随应用分发；拥有授权的素材应由用户在本机导入。
        </p>
      </div>

      <div className="theme-gallery" aria-label="主题列表">
        {(snapshot?.packs ?? []).map((pack) => {
          const active = snapshot?.applied_theme_id === pack.id;
          const selected = snapshot?.selected_theme_id === pack.id;
          return (
            <article
              className={`theme-card${active ? " theme-card--active" : ""}${selected && !active ? " theme-card--selected" : ""}`}
              key={pack.id}
            >
              <div
                className="theme-card__preview"
                style={
                  {
                    "--theme-preview-surface": pack.palette.surface,
                    "--theme-preview-overlay":
                      pack.backdrop.kind === "image"
                        ? pack.backdrop.overlay
                        : pack.palette.surface_strong,
                    "--theme-preview-position":
                      pack.backdrop.kind === "image"
                        ? `${pack.backdrop.focal_x}% ${pack.backdrop.focal_y}%`
                        : "50% 50%",
                  } as CSSProperties
                }
              >
                {pack.category === "local-import" ? (
                  <ThemePreview themeId={pack.id} name={pack.name} />
                ) : (
                  <CompleteThemePreview source={pack.preview_path} name={pack.name} />
                )}
                <span className="theme-rights-badge">
                  {pack.category === "local-import" ? "仅限本机" : "版权已核验"}
                </span>
              </div>
              <div className="theme-card__body">
                <div className="theme-card__title">
                  <div>
                    <span>{CATEGORY_LABELS[pack.category]}</span>
                    <h3>{pack.name}</h3>
                  </div>
                  <i style={{ backgroundColor: pack.palette.accent }} aria-hidden="true" />
                </div>
                <p>{pack.description}</p>
                <dl>
                  <div>
                    <dt>权利方</dt>
                    <dd>{pack.rights.rightsholder}</dd>
                  </div>
                  <div>
                    <dt>{pack.category === "local-import" ? "存储范围" : "核验日期"}</dt>
                    <dd>
                      {pack.category === "local-import" ? "仅当前设备" : pack.rights.reviewed_at}
                    </dd>
                  </div>
                </dl>
                <button
                  className={active ? "button-secondary" : "button-primary"}
                  onClick={(event) => {
                    forceRestartTriggerRef.current = event.currentTarget;
                    void themes.activate(pack.id);
                  }}
                  disabled={active || themes.operation !== null}
                >
                  {active
                    ? "当前主题"
                    : themes.operation === "activate"
                      ? "正在启动并应用…"
                      : "应用主题"}
                </button>
              </div>
            </article>
          );
        })}
      </div>

      <article className="theme-restore-panel">
        <div>
          <span className="eyebrow">SAFE RESTORE</span>
          <h3>恢复官方外观</h3>
          <p>仅移除 Codex Assistant 注入到主任务窗口的样式，不修改 Codex 安装文件或用户数据。</p>
        </div>
        {hasThemeState ? (
          <button
            className="button-secondary"
            onClick={() => void themes.restore()}
            disabled={themes.operation !== null}
          >
            {themes.operation === "restore"
              ? "正在恢复…"
              : paused && !snapshot?.applied_theme_id
                ? "取消主题并保持官方外观"
                : "恢复官方外观"}
          </button>
        ) : (
          <span className="theme-official-status" role="status">
            当前已是官方外观
          </span>
        )}
      </article>

      {themes.pendingForce ? (
        <ForceRestartDialog
          impact={themes.pendingForce}
          busy={themes.operation !== null}
          returnFocus={forceRestartTriggerRef.current}
          onCancel={themes.cancelForceRestart}
          onConfirm={() => void themes.confirmForceRestart()}
        />
      ) : null}
    </section>
  );
}

const CHECK_LABELS: Record<ThemeEnvironmentCheckCode, string> = {
  "supported-windows": "Windows 运行环境",
  "supported-architecture": "处理器架构",
  "official-store-codex": "Microsoft Store 官方 Codex",
  "compatible-adapter": "当前版本适配器",
  "single-codex-window": "Codex 窗口数量",
  "verified-theme-session": "主题控制会话",
  "saved-theme": "主题偏好",
};

function ThemeEnvironmentPanel({ report }: { report: ThemeEnvironmentReport }) {
  const guidance = environmentGuidance(report);
  return (
    <article className={`theme-environment-panel theme-environment-panel--${report.status}`}>
      <div className="theme-environment-panel__heading">
        <div>
          <span className="eyebrow">LOCAL PREFLIGHT</span>
          <h3>本机环境检测</h3>
        </div>
        <span className="theme-environment-version">
          {report.codex_version ? `Codex ${report.codex_version}` : "未检测到 Codex"} ·{" "}
          {report.architecture.toUpperCase()}
          {report.os_build ? ` · Build ${report.os_build}` : ""}
        </span>
      </div>
      <ul className="theme-environment-checks">
        {report.checks.map((check) => (
          <li key={check.code} data-state={check.state}>
            <i aria-hidden="true" />
            <span>{CHECK_LABELS[check.code]}</span>
            <strong>
              {check.state === "pass" ? "通过" : check.state === "action" ? "需处理" : "不可用"}
            </strong>
          </li>
        ))}
      </ul>
      <p className="theme-environment-guidance">{guidance}</p>
      <p className="theme-environment-persistence">
        主题选择会保留；完全关闭并从官方入口重新打开 ChatGPT/Codex
        后，需要回到这里再次点击“应用主题”。
      </p>
    </article>
  );
}

function environmentGuidance(report: ThemeEnvironmentReport): string {
  switch (report.next_action) {
    case "install-codex":
      return "请先从 Microsoft Store 安装官方 Codex，启动一次完成登录后，再返回这里刷新检测。";
    case "close-extra-windows":
      return "检测到多个 Codex 主窗口。请关闭多余的 Codex 窗口，只保留一个后重新检测。";
    case "confirm-restart":
      return "当前官方应用无法在运行后补加受验证的本机主题端口；点击应用后会先显示重启影响并等待你确认。";
    case "launch-codex-for-theme":
      return report.selected_theme_id
        ? "官方应用未运行。点击后会启动官方 ChatGPT/Codex 并应用已保存主题。"
        : "官方应用未运行。选择一套主题后再点击应用。";
    case "update-assistant":
      return `当前官方版本尚未适配（Codex ${report.codex_version ?? "未知版本"}）。请更新 Codex Assistant 或等待兼容更新，官方外观已保留。`;
    case "use-supported-windows":
      return "当前系统不在支持矩阵内。请使用 Windows 10 22H2 或 Windows 11，并安装与 x64/ARM64 架构匹配的版本。";
    case "apply-now":
      return "环境与主题会话均已验证，可以直接切换主题。";
    default:
      return "当前环境暂不支持自动换肤，请按失败项完成处理后刷新。";
  }
}

const MAX_LOCAL_THEME_BYTES = 1_450_000;
const LOCAL_THEME_TYPES = new Set(["image/jpeg", "image/png", "image/webp"]);

function readLocalThemeFile(file: File): Promise<string> {
  if (!LOCAL_THEME_TYPES.has(file.type) || file.size === 0 || file.size > MAX_LOCAL_THEME_BYTES) {
    return Promise.reject(new Error("unsupported local theme image"));
  }
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("local theme image could not be read"));
    });
    reader.addEventListener("error", () =>
      reject(new Error("local theme image could not be read")),
    );
    reader.readAsDataURL(file);
  });
}
