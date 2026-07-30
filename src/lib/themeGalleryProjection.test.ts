import { describe, expect, it } from "vitest";
import type { ThemePack } from "../../shared/theme-types";
import { projectThemeGallery } from "./themeGalleryProjection";

function pack(
  id: string,
  {
    genres = [],
    badges = [],
    publishedAt = "2026-07-01",
    sortOrder = 100,
    category = "abstract",
    rightsStatus = "verified",
  }: {
    genres?: ThemePack["marketplace"] extends infer _
      ? NonNullable<ThemePack["marketplace"]>["genres"]
      : never;
    badges?: NonNullable<ThemePack["marketplace"]>["badges"];
    publishedAt?: string;
    sortOrder?: number;
    category?: ThemePack["category"];
    rightsStatus?: ThemePack["rights"]["status"];
  } = {},
): ThemePack {
  return {
    schema_version: 1,
    minimum_engine_version: 1,
    id,
    name: id,
    description: `${id} description`,
    category,
    marketplace:
      category === "local-import"
        ? undefined
        : {
            genres,
            badges,
            published_at: publishedAt,
            sort_order: sortOrder,
          },
    preview_path: `/themes/${id}.webp`,
    backdrop: { kind: "gradient", angle: 90, colors: ["#111", "#222", "#333"] },
    palette: {
      surface: "#111",
      surface_strong: "#222",
      text: "#fff",
      accent: "#c67d91",
      border: "#444",
    },
    effects: {
      surface_opacity: 80,
      blur_px: 18,
      contrast_percent: 100,
      motion: false,
    },
    assets: [],
    rights: {
      source: "test",
      rightsholder: "Test Author",
      license: "test",
      commercial_redistribution: rightsStatus === "verified",
      attribution: "test",
      reviewed_at: publishedAt,
      manual_signoff: rightsStatus === "verified",
      status: rightsStatus,
    },
  };
}

describe("projectThemeGallery", () => {
  it("applies marketplace and genre filters as one deterministic projection", () => {
    const featuredNature = pack("featured-nature", {
      genres: ["nature"],
      badges: ["popular", "featured"],
      sortOrder: 20,
    });
    const popularCyber = pack("popular-cyber", {
      genres: ["cyber"],
      badges: ["popular"],
      sortOrder: 10,
    });
    const local = pack("local", {
      category: "local-import",
      rightsStatus: "local-only",
    });

    const result = projectThemeGallery({
      packs: [popularCyber, local, featuredNature],
      galleryFilter: "popular",
      genreFilter: "nature",
      favoriteIds: ["featured-nature"],
      appliedThemeId: "featured-nature",
      selectedThemeId: "popular-cyber",
    });

    expect(result).toEqual([
      expect.objectContaining({
        pack: featuredNature,
        active: true,
        selected: false,
        favorite: true,
        official: true,
        editorialBadge: "featured",
      }),
    ]);
  });
});
