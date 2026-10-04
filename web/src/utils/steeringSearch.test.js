import { test } from "node:test";
import assert from "node:assert/strict";
import {
  canSearch,
  resultToReference,
  sectionsToReferences,
  MAX_REFERENCES,
} from "./steeringSearch.js";

const artist = (id, name) => ({ type: "Artist", id, name });
const album = (id, name, artists) => ({
  type: "Album",
  id,
  name,
  artists_ids_names: artists,
});
const track = (id, name, artists) => ({
  type: "Track",
  id,
  name,
  artists_ids_names: artists,
});

test("queries need at least two non-blank characters", () => {
  assert.equal(canSearch(""), false);
  assert.equal(canSearch("a"), false);
  assert.equal(canSearch("  b  "), false);
  assert.equal(canSearch("ab"), true);
  assert.equal(canSearch(" miles "), true);
  assert.equal(canSearch(undefined), false);
});

test("results map to references with artist names as detail", () => {
  assert.deepEqual(resultToReference(artist("a1", "Miles Davis")), {
    entity_type: "artist",
    entity_id: "a1",
    label: "Miles Davis",
    detail: "",
  });
  assert.deepEqual(
    resultToReference(
      track("t1", "So What", [
        ["a1", "Miles Davis"],
        ["a2", "John Coltrane"],
      ]),
    ),
    {
      entity_type: "track",
      entity_id: "t1",
      label: "So What",
      detail: "Miles Davis, John Coltrane",
    },
  );
  assert.equal(resultToReference({ type: "Work", id: "w1", name: "x" }), null);
  assert.equal(resultToReference({ type: "Album", name: "no id" }), null);
});

test("sections keep the primary match first and skip enrichment", () => {
  const sections = [
    { section: "primary_artist", item: artist("a1", "Miles Davis") },
    {
      section: "popular_by",
      target_id: "a1",
      items: [{ id: "t9", name: "Summary only" }],
    },
    {
      section: "more_results",
      items: [
        album("al1", "Kind of Blue", [["a1", "Miles Davis"]]),
        artist("a1", "Miles Davis"),
        { type: "Work", id: "w1", name: "A work" },
        track("t1", "So What", [["a1", "Miles Davis"]]),
      ],
    },
    { section: "done", total_time_ms: 12 },
  ];
  assert.deepEqual(
    sectionsToReferences(sections).map((r) => r.entity_id),
    ["a1", "al1", "t1"],
  );
});

test("references are capped", () => {
  const items = Array.from({ length: MAX_REFERENCES + 5 }, (_, i) =>
    artist(`a${i}`, `Artist ${i}`),
  );
  assert.equal(
    sectionsToReferences([{ section: "results", items }]).length,
    MAX_REFERENCES,
  );
});
