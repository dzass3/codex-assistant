import { useCallback, useState } from "react";

export const THEME_FAVORITES_KEY = "codex-assistant:theme-favorites:v1";

const THEME_ID = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

function readFavorites(): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(THEME_FAVORITES_KEY) ?? "[]");
    if (!Array.isArray(value)) return [];
    return [
      ...new Set(
        value.filter((entry): entry is string => typeof entry === "string" && THEME_ID.test(entry)),
      ),
    ].toSorted();
  } catch {
    return [];
  }
}

function persistFavorites(value: string[]) {
  try {
    localStorage.setItem(THEME_FAVORITES_KEY, JSON.stringify(value));
  } catch {
    // Favorites remain usable for the current session when storage is unavailable.
  }
}

export function useThemeFavorites() {
  const [favoriteIds, setFavoriteIds] = useState(readFavorites);

  const toggleFavorite = useCallback((themeId: string) => {
    if (!THEME_ID.test(themeId)) return;
    setFavoriteIds((current) => {
      const next = current.includes(themeId)
        ? current.filter((id) => id !== themeId)
        : [...current, themeId].toSorted();
      persistFavorites(next);
      return next;
    });
  }, []);

  const isFavorite = useCallback((themeId: string) => favoriteIds.includes(themeId), [favoriteIds]);

  return { favoriteIds, toggleFavorite, isFavorite };
}
