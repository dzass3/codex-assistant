export type AppPage = "live" | "routing" | "themes";

export function AppNavigation({
  active,
  onChange,
}: {
  active: AppPage;
  onChange: (page: AppPage) => void;
}) {
  return (
    <nav className="app-navigation" role="tablist" aria-label="Codex Assistant 功能">
      <button
        type="button"
        role="tab"
        aria-selected={active === "live"}
        className={active === "live" ? "is-active" : undefined}
        onClick={() => onChange("live")}
      >
        实时代理
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={active === "routing"}
        className={active === "routing" ? "is-active" : undefined}
        onClick={() => onChange("routing")}
      >
        Smart Routing
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={active === "themes"}
        className={active === "themes" ? "is-active" : undefined}
        onClick={() => onChange("themes")}
      >
        主题管理
      </button>
    </nav>
  );
}
