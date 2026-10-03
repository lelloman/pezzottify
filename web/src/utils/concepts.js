// Steering concepts: named musical ideas (genres, instruments, moods, eras) that can be
// part of a playback destination mix. See docs/steering-concepts.md.

export const CONCEPT_FAMILIES = Object.freeze([
  { id: "sound_genre", label: "Genres (sound)" },
  { id: "instrument", label: "Instruments" },
  { id: "vocals", label: "Vocals" },
  { id: "mood", label: "Moods" },
  { id: "genre_tag", label: "Genre tags" },
  { id: "recorded", label: "Recorded in" },
  { id: "composed", label: "Composed in" },
]);

const FAMILY_LABELS = Object.fromEntries(
  CONCEPT_FAMILIES.map(({ id, label }) => [id, label]),
);

// Short badge for a concept id when its family is not known (e.g. synced from Android).
const PREFIX_BADGES = {
  audioset: "Sound",
  genre: "Genre tag",
  recorded: "Recorded",
  composed: "Composed",
};

export function familyLabel(family) {
  return FAMILY_LABELS[family] || family || "Concept";
}

// Badge shown next to a destination or source component.
export function componentBadge(component) {
  if (!component) return "";
  if (component.entity_type !== "concept") return component.entity_type;
  if (component.family) return familyLabel(component.family);
  const prefix = String(component.entity_id || "").split(":")[0];
  return PREFIX_BADGES[prefix] || "Concept";
}

// Group concepts by family in the canonical family order, keeping server order inside.
export function groupConcepts(concepts) {
  const groups = new Map(CONCEPT_FAMILIES.map(({ id }) => [id, []]));
  for (const concept of concepts) {
    if (!groups.has(concept.family)) groups.set(concept.family, []);
    groups.get(concept.family).push(concept);
  }
  return [...groups.entries()]
    .filter(([, items]) => items.length)
    .map(([family, items]) => ({ family, label: familyLabel(family), items }));
}

export function matchesConceptQuery(concept, query) {
  const text = query.trim().toLowerCase();
  if (!text) return true;
  return (
    concept.label.toLowerCase().includes(text) ||
    familyLabel(concept.family).toLowerCase().includes(text)
  );
}
