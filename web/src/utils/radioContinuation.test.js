import test from "node:test";
import assert from "node:assert/strict";
import {
  createRadioContinuation,
  editRadioContinuation,
  nextSnapshotBatch,
  appendRadioBatch,
} from "./radioContinuation.js";

const ids = (count) => Array.from({ length: count }, (_, i) => `track-${i}`);
const queue = (snapshot) => ({
  tracksIds: snapshot.slice(0, 10),
  continuation: createRadioContinuation(snapshot.slice(0, 10), snapshot),
});

test("greatest hits consumes ranked batches, including a short final batch", () => {
  let playlist = queue(ids(23));
  const first = nextSnapshotBatch(playlist.continuation);
  assert.deepEqual(first.trackIds, ids(20).slice(10));
  playlist = appendRadioBatch(
    playlist,
    8,
    first.trackIds,
    first.nextIndex,
  ).playlist;
  const last = nextSnapshotBatch(playlist.continuation);
  assert.deepEqual(last.trackIds, ids(23).slice(20));
  playlist = appendRadioBatch(
    playlist,
    18,
    last.trackIds,
    last.nextIndex,
  ).playlist;
  assert.deepEqual(playlist.tracksIds, ids(23));
  assert.equal(playlist.continuation.status, "exhausted");
});

test("edits never reintroduce removed tracks or manually added future tracks", () => {
  let playlist = queue(ids(25));
  playlist.tracksIds = [...playlist.tracksIds.slice(1), "track-12"];
  playlist.continuation = editRadioContinuation(
    playlist.continuation,
    playlist.tracksIds,
    true,
  );
  const next = nextSnapshotBatch(playlist.continuation);
  assert.equal(next.trackIds.includes("track-0"), false);
  assert.equal(next.trackIds.includes("track-12"), false);
  assert.equal(next.trackIds.length, 10);
  assert.equal(next.nextIndex, 21);
  assert.equal(
    editRadioContinuation(playlist.continuation, playlist.tracksIds, false)
      .status,
    "stopped",
  );
});

test("over 500 tracks trims only history and retains session-wide exclusions", () => {
  let playlist = queue(ids(530));
  let currentIndex = 8;
  while (playlist.continuation.status === "active") {
    const batch = nextSnapshotBatch(playlist.continuation);
    const currentId = playlist.tracksIds[currentIndex];
    const result = appendRadioBatch(
      playlist,
      currentIndex,
      batch.trackIds,
      batch.nextIndex,
    );
    assert.equal(result.playlist.tracksIds[result.index], currentId);
    assert.ok(result.playlist.tracksIds.length <= 500);
    playlist = result.playlist;
    currentIndex = playlist.tracksIds.length - 2;
  }
  assert.deepEqual(playlist.continuation.seen_track_ids, ids(530));
  assert.deepEqual(playlist.tracksIds, ids(530).slice(30));
});

test("serialization preserves a stopped session and snapshot position", () => {
  const playlist = queue(ids(100));
  playlist.continuation = editRadioContinuation(
    playlist.continuation,
    playlist.tracksIds,
    false,
  );
  assert.deepEqual(JSON.parse(JSON.stringify(playlist)), playlist);
});

test("short snapshots start exhausted and seeded radios exhaust on empty batches", () => {
  assert.equal(queue(ids(5)).continuation.status, "exhausted");
  const playlist = {
    tracksIds: ids(10),
    continuation: createRadioContinuation(ids(10)),
  };
  assert.equal(
    appendRadioBatch(playlist, 8, []).playlist.continuation.status,
    "exhausted",
  );
  assert.equal(
    appendRadioBatch(playlist, 8, ["track-0", "new", "new"]).playlist.tracksIds
      .length,
    11,
  );
});
