// Visual helpers for the steering page: deterministic colours for concept tiles and
// the hero backdrop, in the spirit of Spotify's "Browse" genre tiles.

// Saturated, mid-dark colours that keep white text readable on a dark theme.
export const TILE_PALETTE = Object.freeze([
  "#8d67ab",
  "#e8115b",
  "#1e3264",
  "#148a08",
  "#e13300",
  "#477d95",
  "#ba5d07",
  "#503750",
  "#0d73ec",
  "#af2896",
  "#27856a",
  "#8c1932",
  "#7358ff",
  "#d84000",
]);

export function hashString(value) {
  let hash = 2166136261;
  for (const char of String(value ?? "")) {
    hash ^= char.codePointAt(0);
    hash = Math.imul(hash, 16777619) >>> 0;
  }
  return hash;
}

// Same id, same colour, on every device and page load.
export function tileColor(id) {
  return TILE_PALETTE[hashString(id) % TILE_PALETTE.length];
}

// "#rrggbb" -> "rgba(r, g, b, alpha)".
export function withAlpha(hex, alpha) {
  const value = String(hex).replace("#", "");
  const r = parseInt(value.slice(0, 2), 16);
  const g = parseInt(value.slice(2, 4), 16);
  const b = parseInt(value.slice(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

export function referenceKey(reference) {
  return `${reference?.entity_type}:${reference?.entity_id}`;
}

// Short title for a set of references: "Kind of Blue", "Kind of Blue + 2 more".
export function mixTitle(references) {
  const list = references || [];
  if (!list.length) return "";
  const first = list[0].label || list[0].entity_id;
  return list.length === 1 ? first : `${first} + ${list.length - 1} more`;
}
