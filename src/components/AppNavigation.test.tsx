import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppNavigation } from "./AppNavigation";

describe("AppNavigation", () => {
  it("switches between Live Agents, Smart Routing, and Themes", () => {
    const onChange = vi.fn();
    render(<AppNavigation active="live" onChange={onChange} />);

    expect(screen.getByRole("tab", { name: "实时代理" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Smart Routing" }));
    expect(onChange).toHaveBeenCalledWith("routing");
    fireEvent.click(screen.getByRole("tab", { name: "主题管理" }));
    expect(onChange).toHaveBeenCalledWith("themes");
  });
});
