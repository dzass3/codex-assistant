import { describe, expect, it } from "vitest";
import { isValidThemeMarketplace } from "../scripts/themes/theme-marketplace-validation.mjs";

const validMarketplace = {
  genres: ["anime", "nature"],
  badges: ["featured", "new"],
  published_at: "2026-07-28",
  sort_order: 10,
};

describe("isValidThemeMarketplace", () => {
  it("accepts the shared marketplace contract and rejects invalid catalog metadata", () => {
    expect(isValidThemeMarketplace(validMarketplace)).toBe(true);
    expect(isValidThemeMarketplace({ ...validMarketplace, genres: ["anime", "unknown"] })).toBe(
      false,
    );
    expect(isValidThemeMarketplace({ ...validMarketplace, badges: ["featured", "featured"] })).toBe(
      false,
    );
    expect(isValidThemeMarketplace({ ...validMarketplace, published_at: "2026-02-30" })).toBe(
      false,
    );
    expect(isValidThemeMarketplace({ ...validMarketplace, sort_order: 0 })).toBe(false);
  });
});
