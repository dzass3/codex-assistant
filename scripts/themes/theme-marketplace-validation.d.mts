export interface ThemeMarketplaceMetadataInput {
  genres?: unknown;
  badges?: unknown;
  published_at?: unknown;
  sort_order?: unknown;
}

export function isValidThemeMarketplace(
  marketplace: ThemeMarketplaceMetadataInput | null | undefined,
): boolean;
