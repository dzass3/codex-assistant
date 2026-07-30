import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { THEME_FAVORITES_KEY, useThemeFavorites } from "./useThemeFavorites";

describe("useThemeFavorites", () => {
  beforeEach(() => localStorage.clear());

  it("persists one favorite across a remount and removes it again", () => {
    const first = renderHook(() => useThemeFavorites());

    act(() => first.result.current.toggleFavorite("wisteria-bride"));
    expect(first.result.current.isFavorite("wisteria-bride")).toBe(true);
    expect(localStorage.getItem(THEME_FAVORITES_KEY)).toBe('["wisteria-bride"]');

    first.unmount();
    const second = renderHook(() => useThemeFavorites());
    expect(second.result.current.isFavorite("wisteria-bride")).toBe(true);

    act(() => second.result.current.toggleFavorite("wisteria-bride"));
    expect(second.result.current.isFavorite("wisteria-bride")).toBe(false);
    expect(localStorage.getItem(THEME_FAVORITES_KEY)).toBe("[]");
  });

  it("deduplicates valid identifiers and ignores corrupt storage", () => {
    localStorage.setItem(
      THEME_FAVORITES_KEY,
      JSON.stringify(["wisteria-bride", "wisteria-bride", "", 42]),
    );
    const valid = renderHook(() => useThemeFavorites());
    expect(valid.result.current.favoriteIds).toEqual(["wisteria-bride"]);
    valid.unmount();

    localStorage.setItem(THEME_FAVORITES_KEY, "{not-json");
    const corrupt = renderHook(() => useThemeFavorites());
    expect(corrupt.result.current.favoriteIds).toEqual([]);
  });
});
