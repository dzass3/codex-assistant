const THEME_GENRES = new Set(["anime", "fantasy", "nature", "cyber", "minimal", "dark", "space"]);
const THEME_BADGES = new Set(["popular", "featured", "new"]);

export function isValidThemeMarketplace(marketplace) {
  return (
    Array.isArray(marketplace?.genres) &&
    marketplace.genres.length > 0 &&
    hasUniqueAllowedValues(marketplace.genres, THEME_GENRES) &&
    Array.isArray(marketplace.badges) &&
    hasUniqueAllowedValues(marketplace.badges, THEME_BADGES) &&
    isCalendarDate(marketplace.published_at) &&
    Number.isSafeInteger(marketplace.sort_order) &&
    marketplace.sort_order > 0
  );
}

function hasUniqueAllowedValues(values, allowed) {
  return new Set(values).size === values.length && values.every((value) => allowed.has(value));
}

function isCalendarDate(value) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  const date = new Date(Date.UTC(year, month - 1, day));
  return (
    date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day
  );
}
