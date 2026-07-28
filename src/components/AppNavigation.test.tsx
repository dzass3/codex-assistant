import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AppNavigation } from "./AppNavigation";

describe("AppNavigation", () => {
  it("exposes exactly the two approved product pages", () => {
    const onChange = vi.fn();
    render(<AppNavigation active="monitor" onChange={onChange} />);

    const tabs = screen.getAllByRole("tab");
    expect(tabs).toHaveLength(2);
    expect(screen.getByRole("tab", { name: "实时代理" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: "一键换肤" })).toHaveAttribute("aria-selected", "false");
    expect(screen.queryByText(/Smart Routing/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "一键换肤" }));
    expect(onChange).toHaveBeenCalledWith("themes");
  });
});
