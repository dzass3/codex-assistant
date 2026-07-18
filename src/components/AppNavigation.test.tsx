import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppNavigation } from "./AppNavigation";

describe("AppNavigation", () => {
  it("switches between Live Agents and Smart Routing", () => {
    const onChange = vi.fn();
    render(<AppNavigation active="live" onChange={onChange} />);

    expect(screen.getByRole("tab", { name: "实时代理" })).toHaveAttribute("aria-selected", "true");
    fireEvent.click(screen.getByRole("tab", { name: "Smart Routing" }));
    expect(onChange).toHaveBeenCalledWith("routing");
  });
});
