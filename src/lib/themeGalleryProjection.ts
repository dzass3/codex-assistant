import type { ThemeEditorialBadge, ThemeGenre, ThemePack } from "../../shared/theme-types";

export type GalleryFilter = "all" | "popular" | "favorites" | "latest" | "official";

export interface ThemeGalleryProjectionInput {
  packs: ThemePack[];
  galleryFilter: GalleryFilter;
  genreFilter: ThemeGenre | null;
  favoriteIds: string[];
  appliedThemeId: string | null;
  selectedThemeId: string | null;
}

export interface ThemeGalleryEntry {
  pack: ThemePack;
  active: boolean;
  selected: boolean;
  favorite: boolean;
  official: boolean;
  editorialBadge: ThemeEditorialBadge | null;
}

const BADGE_PRIORITY: ThemeEditorialBadge[] = ["featured", "popular", "new"];

export function projectThemeGallery({
  packs,
  galleryFilter,
  genreFilter,
  favoriteIds,
  appliedThemeId,
  selectedThemeId,
}: ThemeGalleryProjectionInput): ThemeGalleryEntry[] {
  const favorites = new Set(favoriteIds);

  return packs
    .filter((pack) => matchesGalleryFilter(pack, galleryFilter, favorites))
    .filter((pack) => genreFilter === null || pack.marketplace?.genres.includes(genreFilter))
    .toSorted((left, right) => compareThemes(left, right, galleryFilter))
    .map((pack) => ({
      pack,
      active: appliedThemeId === pack.id,
      selected: selectedThemeId === pack.id && appliedThemeId !== pack.id,
      favorite: favorites.has(pack.id),
      official: pack.rights.status === "verified",
      editorialBadge:
        BADGE_PRIORITY.find((badge) => pack.marketplace?.badges.includes(badge)) ?? null,
    }));
}

function matchesGalleryFilter(
  pack: ThemePack,
  filter: GalleryFilter,
  favoriteIds: Set<string>,
): boolean {
  switch (filter) {
    case "popular":
      return pack.marketplace?.badges.includes("popular") ?? false;
    case "favorites":
      return favoriteIds.has(pack.id);
    case "latest":
      return (
        pack.category === "local-import" || (pack.marketplace?.badges.includes("new") ?? false)
      );
    case "official":
      return pack.rights.status === "verified";
    case "all":
      return true;
  }
}

function compareThemes(left: ThemePack, right: ThemePack, filter: GalleryFilter): number {
  if (filter === "latest") {
    const date = themePublishedAt(right).localeCompare(themePublishedAt(left));
    if (date !== 0) return date;
  }
  return (
    (left.marketplace?.sort_order ?? Number.MAX_SAFE_INTEGER) -
    (right.marketplace?.sort_order ?? Number.MAX_SAFE_INTEGER)
  );
}

function themePublishedAt(pack: ThemePack): string {
  return pack.marketplace?.published_at ?? pack.rights.reviewed_at;
}
