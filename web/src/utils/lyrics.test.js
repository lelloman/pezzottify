import test from "node:test";
import assert from "node:assert/strict";
import { parseSyncedLyrics, activeLyricIndex } from "./lyrics.js";

test("LRC timestamps preserve repeated verses, fractions, offsets and instrumental gaps", () => {
  const lines = parseSyncedLyrics(
    "[ar:Example]\n[offset:100]\n[00:02.50][00:12.500]Repeated line\r\n[00:01.1]First\n[00:03.00]\n[bad]Ignored",
  );
  assert.deepEqual(lines, [
    { time: 1, text: "First" },
    { time: 2.4, text: "Repeated line" },
    { time: 2.9, text: "" },
    { time: 12.4, text: "Repeated line" },
  ]);
  assert.equal(activeLyricIndex(lines, 0), -1);
  assert.equal(activeLyricIndex(lines, 2.4), 1);
  assert.equal(activeLyricIndex(lines, 3), 2);
  assert.equal(activeLyricIndex(lines, 99), 3);
  assert.equal(activeLyricIndex(lines, 1), 0); // Backward seeks.
});

test("plain text, metadata and malformed timestamps do not become timed lines", () => {
  assert.deepEqual(parseSyncedLyrics(null), []);
  assert.deepEqual(
    parseSyncedLyrics("Plain lyrics\n[ti:Title]\n[00:99]Bad"),
    [],
  );
  assert.equal(activeLyricIndex([], 42), -1);
  assert.equal(activeLyricIndex([{ time: 0, text: "Intro" }], NaN), -1);
});
