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
});
