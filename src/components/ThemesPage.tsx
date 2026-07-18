import type { ThemeCategory } from "../../shared/theme-types";
import { useTheme } from "../hooks/useTheme";

const CATEGORY_LABELS: Record<ThemeCategory, string> = {
  abstract: "原创抽象",
  "original-character": "原创角色",
  "project-showcase": "项目展示",
  "local-import": "仅限本地导入",
};

export function ThemesPage() {
  const themes = useTheme();
  const snapshot = themes.snapshot;
  const ready = snapshot?.session_status === "ready";

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
          <span className="eyebrow">RIGHTS-AUDITED ONE-CLICK SKINS</span>
          <h2>主题管理</h2>
          <p>选择版权已核验的声明式主题，一键应用到当前 Codex 窗口。</p>
        </div>
        <button className="icon-button" onClick={themes.refresh} disabled={themes.refreshing}>
          {themes.refreshing ? "刷新中" : "刷新状态"}
        </button>
      </div>

      {themes.error ? <div className="global-error">{themes.error}</div> : null}

      <article className="theme-session-panel">
        <div>
          <span className="eyebrow">VERIFIED LOCAL SESSION</span>
          <h3>{ready ? "主题会话已就绪" : "启动主题会话"}</h3>
          <p>
            首次启用会安全重启一次 Codex，以建立仅绑定本机、同一 Windows 用户和官方 Codex
            进程的控制会话。运行中的原生子代理会阻止重启，任务不会被中断。
          </p>
        </div>
        {!ready ? (
          <button
            className="button-primary"
            onClick={() => void themes.startSession()}
            disabled={themes.operation !== null}
          >
            {themes.operation === "start-session" ? "正在安全启动…" : "启动主题会话"}
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

      <div className="theme-gallery" aria-label="内置主题">
        {(snapshot?.packs ?? []).map((pack) => {
          const active = snapshot?.active_theme_id === pack.id;
          return (
            <article className={`theme-card${active ? " theme-card--active" : ""}`} key={pack.id}>
              <div className="theme-card__preview">
                <img src={pack.preview_path} alt={`${pack.name} 主题预览`} />
                <span className="theme-rights-badge">版权已核验</span>
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
                    <dt>核验日期</dt>
                    <dd>{pack.rights.reviewed_at}</dd>
                  </div>
                </dl>
                <button
                  className={active ? "button-secondary" : "button-primary"}
                  onClick={() => void themes.apply(pack.id)}
                  disabled={!ready || active || themes.operation !== null}
                >
                  {active ? "当前主题" : themes.operation === "apply" ? "正在应用…" : "应用主题"}
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
          <p>仅移除 Codex Assistant 注入的样式，不修改 Codex 安装文件或用户数据。</p>
        </div>
        <button
          className="button-secondary"
          onClick={() => void themes.restore()}
          disabled={!snapshot?.active_theme_id || themes.operation !== null}
        >
          {themes.operation === "restore" ? "正在恢复…" : "恢复官方外观"}
        </button>
      </article>
    </section>
  );
}
