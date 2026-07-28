import { useState } from "react";
import { AppNavigation, type AppPage } from "./components/AppNavigation";
import { MonitorPage } from "./components/MonitorPage";
import { ThemesPage } from "./components/ThemesPage";
import { PRODUCT_NAME, PRODUCT_TAGLINE } from "./config";

const LAST_PAGE_KEY = "codex-assistant:last-page:v1";

function readInitialPage(): AppPage {
  try {
    return localStorage.getItem(LAST_PAGE_KEY) === "monitor" ? "monitor" : "themes";
  } catch {
    return "themes";
  }
}

export function App() {
  const [page, setPage] = useState<AppPage>(readInitialPage);

  const changePage = (next: AppPage) => {
    setPage(next);
    try {
      localStorage.setItem(LAST_PAGE_KEY, next);
    } catch {
      // Page selection remains usable when local storage is unavailable.
    }
  };

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            <i />
            <i />
            <i />
          </span>
          <div>
            <h1>{PRODUCT_NAME}</h1>
            <p>{PRODUCT_TAGLINE}</p>
          </div>
        </div>
        <div className="topbar-actions">
          <span className="local-only-note">
            <i className="pulse-dot" aria-hidden="true" />
            只读观察 · 主题仅在本机处理
          </span>
        </div>
      </header>

      <AppNavigation active={page} onChange={changePage} />
      {page === "monitor" ? <MonitorPage /> : <ThemesPage />}
    </main>
  );
}
