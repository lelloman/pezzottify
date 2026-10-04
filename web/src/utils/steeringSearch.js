// Steering reference search on top of the shared streaming search (the path the main
// search screen uses). Pure helpers so the mapping and the query rule are testable.
import { sectionsToResults } from "../services/streamingSearch.js";

// One- and two-letter queries are the most expensive against the search index and
// rarely useful for picking a reference, so the picker waits for at least this many.
export const MIN_QUERY_LENGTH = 2;
export const SEARCH_DEBOUNCE_MS = 500;
export const MAX_REFERENCES = 30;

const TYPES = { Artist: "artist", Album: "album", Track: "track" };

export function canSearch(query) {
  return (query || "").trim().length >= MIN_QUERY_LENGTH;
}

const artistNames = (result) =>
  (result.artists_ids_names || [])
    .map((entry) => (Array.isArray(entry) ? entry[1] : entry?.name))
    .filter(Boolean)
    .join(", ");

// Only artists, albums and tracks can be steering references.
export function resultToReference(result) {
  const entityType = TYPES[result?.type];
  if (!entityType || !result.id) return null;
  return {
    entity_type: entityType,
    entity_id: result.id,
    label: result.name || result.id,
    detail: entityType === "artist" ? "" : artistNames(result),
  };
}

// Streamed sections in arrival order: primary matches first, then the remaining
// results; enrichment sections (popular tracks, albums by, related) are skipped.
export function sectionsToReferences(sections) {
  return sectionsToResults(sections)
    .map(resultToReference)
    .filter(Boolean)
    .slice(0, MAX_REFERENCES);
}
