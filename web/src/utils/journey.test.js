import test from "node:test";
import assert from "node:assert/strict";
import {
  JOURNEY_DEFAULTS,
  RESET_JOURNEY,
  displayedFollow,
  displayedVariety,
  followPartial,
  hasCustomJourney,
  journeyPartial,
  listenForLabel,
  varietyPartial,
} from "./journey.js";

test("variety writes diversity and randomness together and clears mode", () => {
  assert.deepEqual(varietyPartial("0.6"), {
    diversity: 0.6,
    randomness: 0.6,
    mode: null,
  });
});

test("every journey write clears mode", () => {
  assert.deepEqual(followPartial(0.5), { recency_weight: 0.5, mode: null });
  assert.deepEqual(journeyPartial({ away: [], mode: "explore" }), {
    away: [],
    mode: null,
  });
});

test("displayed values fall back to server defaults", () => {
  assert.equal(displayedFollow({}), JOURNEY_DEFAULTS.follow);
  assert.equal(displayedFollow({ recency_weight: 0 }), 0);
  assert.equal(displayedVariety({}), JOURNEY_DEFAULTS.variety);
  assert.equal(displayedVariety({ diversity: 0.2, randomness: 0.6 }), 0.4);
  assert.equal(displayedVariety({ diversity: 0.9 }), (0.9 + 0.3) / 2);
});

test("listen-for labels hide namespace names", () => {
  assert.equal(listenForLabel("musicfm.mean.v1", "Sound profile"), "Overall sound");
  assert.equal(listenForLabel("ast.audioset.v2"), "Audio scene");
  assert.equal(listenForLabel("ast.instruments.v1"), "Instruments");
  assert.equal(listenForLabel("custom.ns", "Custom"), "Custom");
});

test("reset clears every setting and custom detection ignores mode", () => {
  assert.equal(hasCustomJourney(RESET_JOURNEY), false);
  assert.equal(hasCustomJourney({ mode: "explore" }), false);
  assert.equal(hasCustomJourney({ away: [{ entity_type: "track" }] }), true);
  assert.equal(hasCustomJourney({ diversity: 0.4 }), true);
});
