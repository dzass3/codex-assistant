import { render, screen } from "@testing-library/react";
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

describe("App", () => {
  it("renders the Codex Assistant product identity", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Codex Assistant" })).toBeInTheDocument();
    expect(screen.getByText("原生代理路由、模型观察与主题管理")).toBeInTheDocument();
  });
});
