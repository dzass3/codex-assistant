import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("./hooks/useMonitor", () => ({
  useMonitor: () => ({
    snapshot: null,
    settings: null,
    loading: false,
    refreshing: false,
    error: null,
    connected: false,
    refresh: vi.fn(),
    setCodexHome: vi.fn(),
  }),
}));

vi.mock("./hooks/useRouting", () => ({
  useRouting: () => ({
    snapshot: null,
    loading: false,
    refreshing: false,
    degraded: true,
    error: "Smart Routing 尚未就绪",
    connected: false,
    operation: null,
    receipt: null,
    refresh: vi.fn(),
    install: vi.fn(),
    restore: vi.fn(),
    requestRestart: vi.fn(),
    beginPreflight: vi.fn(),
    setRootEnabled: vi.fn(),
  }),
}));

vi.mock("./hooks/useTheme", () => ({
  useTheme: () => ({
    snapshot: {
      contract_version: 2,
      session_status: "inactive",
      selected_theme_id: null,
      applied_theme_id: null,
      packs: [],
    },
    loading: false,
    refreshing: false,
    degraded: false,
    error: null,
    connected: true,
    operation: null,
    receipt: null,
    refresh: vi.fn(),
    startSession: vi.fn(),
    apply: vi.fn(),
    restore: vi.fn(),
  }),
}));

describe("App", () => {
  it("renders the Codex Assistant product identity", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Codex Assistant" })).toBeInTheDocument();
    expect(screen.getByText("原生代理路由、模型观察与主题管理")).toBeInTheDocument();
  });

  it("opens Smart Routing in the same application window", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("tab", { name: "Smart Routing" }));

    expect(screen.getByRole("heading", { name: "Smart Routing" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "当前任务与子代理" })).not.toBeInTheDocument();
  });

  it("opens theme management in the same application window", () => {
    render(<App />);

    fireEvent.click(screen.getByRole("tab", { name: "主题管理" }));

    expect(screen.getByRole("heading", { name: "主题管理" })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "当前任务与子代理" })).not.toBeInTheDocument();
  });
});
