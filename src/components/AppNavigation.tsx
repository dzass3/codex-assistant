export type AppPage = "monitor" | "themes";

interface AppNavigationProps {
  active: AppPage;
  onChange: (page: AppPage) => void;
}

export function AppNavigation({ active, onChange }: AppNavigationProps) {
  return (
    <nav className="app-navigation" role="tablist" aria-label="Codex Assistant 功能">
      <button
        type="button"
        role="tab"
        aria-selected={active === "monitor"}
        className={active === "monitor" ? "is-active" : undefined}
        onClick={() => onChange("monitor")}
      >
        实时代理
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={active === "themes"}
        className={active === "themes" ? "is-active" : undefined}
        onClick={() => onChange("themes")}
      >
        一键换肤
      </button>
    </nav>
  );
}
