import test from "node:test";
import assert from "node:assert/strict";
import {
  componentBadge,
  familyLabel,
  groupConcepts,
  matchesConceptQuery,
} from "./concepts.js";

test("concept families have readable labels and badges", () => {
  assert.equal(familyLabel("sound_genre"), "Genres (sound)");
  assert.equal(familyLabel("composed"), "Composed in");
  assert.equal(
    componentBadge({
      entity_type: "concept",
      entity_id: "audioset:Jazz",
      family: "sound_genre",
    }),
    "Genres (sound)",
  );
  assert.equal(
    componentBadge({ entity_type: "concept", entity_id: "recorded:1960s" }),
    "Recorded",
  );
  assert.equal(
    componentBadge({ entity_type: "artist", entity_id: "a1" }),
    "artist",
  );
});

test("concepts are grouped in family order and searchable by label or family", () => {
  const concepts = [
    { id: "genre:psytrance", family: "genre_tag", label: "psytrance" },
    { id: "audioset:Piano", family: "instrument", label: "Piano" },
    { id: "audioset:Jazz", family: "sound_genre", label: "Jazz" },
  ];
  assert.deepEqual(
    groupConcepts(concepts).map((group) => group.family),
    ["sound_genre", "instrument", "genre_tag"],
  );
  assert.ok(matchesConceptQuery(concepts[1], "pia"));
  assert.ok(matchesConceptQuery(concepts[1], "instru"));
  assert.ok(!matchesConceptQuery(concepts[1], "jazz"));
});
