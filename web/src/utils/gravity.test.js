import test from "node:test";
import assert from "node:assert/strict";
import {
  AUTO_IDS_CAP,
  MAX_COMPONENTS,
  addDestinationComponent,
  removeDestinationComponent,
  setDestinationComponentWeight,
  SOURCE_SAMPLE_CAP,
  appendAuto,
  applyDiagnostics,
  arrive,
  buildContinuationRequest,
  create,
  isAuto,
  markUserAdded,
  normalize,
  noteRemoved,
  progress,
  sampleEvenly,
  setDestination,
  setKnobs,
  setSource,
  setStepsTotal,
} from "./gravity.js";

const ids = (count, prefix = "t") =>
  Array.from({ length: count }, (_, i) => `${prefix}${i}`);
const artist = { entity_type: "artist", entity_id: "a1", label: "Artist" };
const artistMix = [{ ...artist, weight: 1 }];
const jazz = {
  entity_type: "concept",
  entity_id: "audioset:Jazz",
  label: "Jazz",
};

test("create returns the documented defaults and independent copies", () => {
  const first = create();
  const second = create();
  assert.deepEqual(first, {
    v: 2,
    source: { kind: "queue" },
    auto_track_ids: [],
    destination: null,
    steps_total: 10,
    steps_done: 0,
    knobs: {
      recency_weight: null,
      criteria: null,
      diversity: null,
      randomness: null,
      mode: null,
      away: [],
    },
    last_diagnostics: null,
  });
  first.auto_track_ids.push("x");
  assert.deepEqual(second.auto_track_ids, []);
  assert.equal(JSON.parse(JSON.stringify(first)).v, 2);
});

test("normalize fills missing keys from persisted or remote objects", () => {
  assert.deepEqual(normalize(null), create());
  const partial = normalize({ auto_track_ids: ["a"], steps_total: 5 });
  assert.deepEqual(partial.auto_track_ids, ["a"]);
  assert.equal(partial.steps_total, 5);
  assert.deepEqual(partial.source, { kind: "queue" });
  assert.equal(partial.knobs.recency_weight, null);
  assert.deepEqual(partial.knobs.away, []);
});

test("appendAuto records provenance, dedupes and caps the list", () => {
  let g = appendAuto(create(), ["s1", "s2"]);
  g = appendAuto(g, ["s2", "s3"]);
  assert.deepEqual(g.auto_track_ids, ["s1", "s2", "s3"]);
  assert.equal(isAuto(g, "s2"), true);
  assert.equal(isAuto(g, "u1"), false);
  assert.equal(g.steps_done, 0, "no destination: steps do not advance");
  const capped = appendAuto(create(), ids(AUTO_IDS_CAP + 10));
  assert.equal(capped.auto_track_ids.length, AUTO_IDS_CAP);
  assert.equal(capped.auto_track_ids[0], "t10");
});

test("markUserAdded promotes a suggestion to the source, noteRemoved keeps exclusion", () => {
  const g = appendAuto(create(), ["s1", "s2"]);
  const promoted = markUserAdded(g, ["s1", "u1"]);
  assert.deepEqual(promoted.auto_track_ids, ["s2"]);
  assert.equal(markUserAdded(g, ["u9"]), g, "untouched when nothing matches");
  assert.deepEqual(noteRemoved(g, "s1").auto_track_ids, ["s1", "s2"]);
});

test("destination steps advance per appended track and arrive at the total", () => {
  let g = setDestination(create(), artist, 3);
  assert.equal(g.steps_total, 3);
  assert.equal(g.steps_done, 0);
  assert.equal(progress(g), 0);
  g = appendAuto(g, ["s1", "s2"]);
  assert.equal(g.steps_done, 2);
  assert.ok(Math.abs(progress(g) - 2 / 3) < 1e-9);
  assert.deepEqual(g.destination, artistMix);
  g = appendAuto(g, ["s3", "s4"]);
  assert.equal(g.destination, null, "arrived");
  assert.deepEqual(g.source, {
    kind: "references",
    references: [{ ...artist, weight: 1 }],
  });
  assert.equal(g.steps_done, 0);
  assert.equal(g.steps_total, 3);
  assert.deepEqual(g.auto_track_ids, ["s1", "s2", "s3", "s4"]);
  assert.equal(progress(g), 0);
});

test("setDestination resets steps, clearing keeps the source, arrive without destination is a no-op", () => {
  let g = appendAuto(setDestination(create(), artist, 4), ["s1"]);
  g = setDestination(g, { entity_type: "track", entity_id: "t9" });
  assert.equal(g.steps_done, 0);
  assert.equal(g.steps_total, 4, "steps total kept when not given");
  assert.equal(g.destination.length, 1, "replaces the whole mix");
  assert.equal(g.destination[0].label, "t9", "label falls back to the id");
  const cleared = setDestination(g, null);
  assert.equal(cleared.destination, null);
  assert.deepEqual(cleared.source, { kind: "queue" });
  assert.equal(arrive(cleared), cleared);
  assert.equal(setDestination(create(), artist, 0).steps_total, 1);
});

test("setStepsTotal clamps to one and arrives when already past the total", () => {
  const g = appendAuto(setDestination(create(), artist, 10), ["s1", "s2"]);
  assert.equal(setStepsTotal(g, 0).steps_total, 1);
  const arrived = setStepsTotal(g, 2);
  assert.equal(arrived.destination, null);
  assert.equal(arrived.source.kind, "references");
  const notYet = setStepsTotal(g, 3);
  assert.deepEqual(notYet.destination, artistMix);
  assert.equal(notYet.steps_total, 3);
});

test("setSource, setKnobs and applyDiagnostics are shallow updates", () => {
  const refs = setSource(create(), {
    kind: "references",
    references: [{ entity_type: "album", entity_id: "al1" }],
  });
  assert.deepEqual(refs.source.references, [
    { entity_type: "album", entity_id: "al1", weight: 1 },
  ]);
  assert.deepEqual(setSource(refs, { kind: "queue" }).source, {
    kind: "queue",
  });
  const knobs = setKnobs(create(), { recency_weight: 0.5, mode: "similar" });
  assert.equal(knobs.knobs.recency_weight, 0.5);
  assert.equal(knobs.knobs.mode, "similar");
  assert.equal(knobs.knobs.diversity, null);
  const diag = applyDiagnostics(create(), { progress: 0.5 }, 123);
  assert.deepEqual(diag.last_diagnostics, { progress: 0.5, at: 123 });
  assert.equal(applyDiagnostics(diag, null).last_diagnostics, null);
});

test("sampleEvenly keeps order and the first element with an even stride", () => {
  const list = ids(1000);
  assert.equal(sampleEvenly(list, 1000), list);
  const sample = sampleEvenly(list, SOURCE_SAMPLE_CAP);
  assert.equal(sample.length, SOURCE_SAMPLE_CAP);
  assert.equal(sample[0], "t0");
  assert.equal(sample.at(-1), "t995");
  const indexes = sample.map((id) => Number(id.slice(1)));
  for (let i = 1; i < indexes.length; i++)
    assert.ok(indexes[i] > indexes[i - 1]);
});

test("buildContinuationRequest anchors on user-chosen tracks and excludes suggestions", () => {
  const queue = ["u0", "u1", "u2", "s1", "s2"];
  const g = appendAuto(create(), ["s1", "s2", "removed"]);
  const request = buildContinuationRequest(g, queue, 3, 2);
  assert.deepEqual(request, {
    context_track_ids: queue,
    recent_track_ids: ["u0", "u1", "u2", "s1"],
    exclude_track_ids: ["u0", "u1", "u2", "s1", "s2", "removed"],
    count: 2,
    source_track_ids: ["u0", "u1", "u2"],
  });
  assert.equal("recency_weight" in request, false);
  assert.equal("destination" in request, false);
});

test("buildContinuationRequest takes the last ten legacy ids, five recent, and samples long sources", () => {
  const queue = ids(500, "u");
  const request = buildContinuationRequest(create(), queue, 499, 1);
  assert.deepEqual(request.context_track_ids, queue.slice(-10));
  assert.deepEqual(request.recent_track_ids, queue.slice(-5));
  assert.equal(request.source_track_ids.length, SOURCE_SAMPLE_CAP);
  assert.equal(request.source_track_ids[0], "u0");
  const early = buildContinuationRequest(create(), ["a", "b", "c"], 1, 1);
  assert.deepEqual(early.recent_track_ids, ["a", "b"]);
  const noIndex = buildContinuationRequest(create(), ["a", "b"], null, 1);
  assert.deepEqual(noIndex.recent_track_ids, ["a", "b"]);
});

test("buildContinuationRequest carries references, destination, progress and knobs when set", () => {
  let g = setDestination(create(), artist, 4);
  g = appendAuto(g, ["s1"]);
  g = setKnobs(g, {
    recency_weight: 0.1,
    criteria: [{ namespace: "musicfm.mean.v1", weight: 1 }],
    diversity: 0.4,
    randomness: 0,
    mode: "similar",
    away: [{ entity_type: "track", entity_id: "bad", weight: 2, label: "x" }],
  });
  const request = buildContinuationRequest(g, ["u0", "s1"], 1, 1);
  assert.deepEqual(request.destination, [
    { entity_type: "artist", entity_id: "a1", weight: 1 },
  ]);
  assert.equal(request.progress, 0.25);
  assert.equal(request.recency_weight, 0.1);
  assert.deepEqual(request.criteria, [
    { namespace: "musicfm.mean.v1", weight: 1 },
  ]);
  assert.equal(request.diversity, 0.4);
  assert.equal(request.randomness, 0);
  assert.equal(request.mode, "similar");
  assert.deepEqual(request.away, [
    { entity_type: "track", entity_id: "bad", weight: 2 },
  ]);

  const arrived = buildContinuationRequest(arrive(g), ["u0", "s1"], 1, 1);
  assert.deepEqual(arrived.source_references, [
    { entity_type: "artist", entity_id: "a1", weight: 1 },
  ]);
  assert.equal("source_track_ids" in arrived, false);
  assert.equal("destination" in arrived, false);
});

test("destination mixes: add, merge, weight, remove and cap", () => {
  let g = setDestination(create(), artist, 5);
  g = appendAuto(g, ["s1"]);
  g = addDestinationComponent(g, { ...jazz, weight: 0.5 });
  assert.deepEqual(
    g.destination.map((c) => [c.entity_id, c.weight]),
    [
      ["a1", 1],
      ["audioset:Jazz", 0.5],
    ],
  );
  assert.equal(g.steps_done, 1, "adding a component keeps progress");
  g = addDestinationComponent(g, { ...jazz, weight: 2 });
  assert.equal(g.destination.length, 2, "same entity merges");
  assert.equal(g.destination[1].weight, 2);
  g = setDestinationComponentWeight(g, jazz, 0.7);
  assert.equal(g.destination[1].weight, 0.7);
  assert.equal(
    setDestinationComponentWeight(g, jazz, 0),
    g,
    "weights stay positive",
  );
  g = removeDestinationComponent(g, artist);
  assert.deepEqual(
    g.destination.map((c) => c.entity_id),
    ["audioset:Jazz"],
  );
  const empty = removeDestinationComponent(g, jazz);
  assert.equal(
    empty.destination,
    null,
    "removing the last component clears it",
  );

  let full = create();
  for (let i = 0; i < MAX_COMPONENTS + 3; i++) {
    full = addDestinationComponent(full, {
      entity_type: "track",
      entity_id: `t${i}`,
    });
  }
  assert.equal(full.destination.length, MAX_COMPONENTS);
  const capped = setDestination(
    create(),
    ids(12).map((id) => ({ entity_type: "track", entity_id: id })),
  );
  assert.equal(capped.destination.length, MAX_COMPONENTS);
});

test("arriving at a mix turns the whole mix into the source", () => {
  let g = setDestination(create(), [artist, { ...jazz, weight: 0.5 }], 2);
  g = appendAuto(g, ["s1", "s2"]);
  assert.equal(g.destination, null);
  assert.deepEqual(g.source, {
    kind: "references",
    references: [
      { ...artist, weight: 1 },
      { ...jazz, weight: 0.5 },
    ],
  });
  const request = buildContinuationRequest(g, ["u0", "s1", "s2"], 2, 1);
  assert.deepEqual(request.source_references, [
    { entity_type: "artist", entity_id: "a1", weight: 1 },
    { entity_type: "concept", entity_id: "audioset:Jazz", weight: 0.5 },
  ]);
});

test("mix requests send every component with its weight", () => {
  const g = setDestination(create(), [artist, { ...jazz, weight: 0.5 }], 4);
  const request = buildContinuationRequest(g, ["u0"], 0, 1);
  assert.deepEqual(request.destination, [
    { entity_type: "artist", entity_id: "a1", weight: 1 },
    { entity_type: "concept", entity_id: "audioset:Jazz", weight: 0.5 },
  ]);
  assert.equal(request.progress, 0);
});

test("v1 single-object destinations are dropped, not migrated", () => {
  const legacy = normalize({
    v: 1,
    destination: artist,
    steps_total: 7,
    steps_done: 3,
  });
  assert.equal(legacy.v, 2);
  assert.equal(legacy.destination, null);
  assert.equal(progress(legacy), 0);
  assert.equal(
    "destination" in buildContinuationRequest(legacy, ["u0"], 0, 1),
    false,
  );
});
