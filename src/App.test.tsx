import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

vi.mock("./components/ThemesPage", () => ({
  ThemesPage: () => <section aria-label="主题页面">主题内容</section>,
}));
vi.mock("./components/MonitorPage", () => ({
  MonitorPage: () => <section aria-label="监控页面">监控内容</section>,
}));

describe("App", () => {
  beforeEach(() => localStorage.clear());

  it("renders the Codex Assistant product identity", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Codex Assistant" })).toBeInTheDocument();
    expect(screen.getByText(/只读代理观察/)).toBeInTheDocument();
  });

  it("defaults a first launch to themes and persists a deliberate page switch", () => {
    render(<App />);

    expect(screen.getByLabelText("主题页面")).toBeInTheDocument();
    expect(screen.queryByLabelText("监控页面")).not.toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveClass("app-shell--themes");

    fireEvent.click(screen.getByRole("tab", { name: "实时代理" }));
    expect(screen.getByLabelText("监控页面")).toBeInTheDocument();
    expect(screen.getByRole("main")).toHaveClass("app-shell--monitor");
    expect(localStorage.getItem("codex-assistant:last-page:v1")).toBe("monitor");
    expect(screen.queryByText("Smart Routing")).not.toBeInTheDocument();
  });

  it("restores a valid page preference and rejects an unknown one", () => {
    localStorage.setItem("codex-assistant:last-page:v1", "monitor");
    const view = render(<App />);
    expect(screen.getByLabelText("监控页面")).toBeInTheDocument();

    view.unmount();
    localStorage.setItem("codex-assistant:last-page:v1", "routing");
    render(<App />);
    expect(screen.getByLabelText("主题页面")).toBeInTheDocument();
  });
});
