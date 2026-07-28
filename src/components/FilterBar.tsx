import type { MonitorFilters } from "./AgentTree";

interface FilterBarProps {
  filters: MonitorFilters;
  models: string[];
  projects: string[];
  onChange: (filters: MonitorFilters) => void;
}

export function FilterBar({ filters, models, projects, onChange }: FilterBarProps) {
  const update = <K extends keyof MonitorFilters>(key: K, value: MonitorFilters[K]) =>
    onChange({ ...filters, [key]: value });

  return (
    <div className="filter-bar">
      <label className="search-field">
        <span aria-hidden="true">⌕</span>
        <input
          aria-label="搜索代理"
          value={filters.query}
          onChange={(event) => update("query", event.target.value)}
          placeholder="搜索代理、角色或模型"
        />
      </label>
      <label>
        <span className="sr-only">模型</span>
        <select value={filters.model} onChange={(event) => update("model", event.target.value)}>
          <option value="all">所有模型</option>
          {models.map((model) => (
            <option key={model} value={model}>
              {model}
            </option>
          ))}
        </select>
      </label>
      <label>
        <span className="sr-only">项目</span>
        <select value={filters.project} onChange={(event) => update("project", event.target.value)}>
          <option value="all">所有项目</option>
          {projects.map((project) => (
            <option key={project} value={project}>
              {project}
            </option>
          ))}
        </select>
      </label>
      <div className="segmented" aria-label="代理范围">
        <button
          type="button"
          className={filters.activeOnly ? "is-active" : ""}
          onClick={() => update("activeOnly", true)}
        >
          活跃
        </button>
        <button
          type="button"
          className={!filters.activeOnly ? "is-active" : ""}
          onClick={() => update("activeOnly", false)}
        >
          全部
        </button>
      </div>
    </div>
  );
}
