import assert from "node:assert/strict";
import test from "node:test";
import {
  canRequestAlbumDownload,
  canRequestTrackDownload,
} from "./downloadRequests.js";

test("album requests use stored availability regardless of proxy playback", () => {
  for (const availability of ["missing", "partial"]) {
    assert.equal(canRequestAlbumDownload(true, availability), true);
    assert.equal(canRequestAlbumDownload(false, availability), false);
  }
  for (const availability of ["complete", undefined]) {
    assert.equal(canRequestAlbumDownload(true, availability), false);
  }
});

test("track requests preserve permissions and exclude stored or fetching tracks", () => {
  for (const availability of ["unavailable", "missing"]) {
    assert.equal(canRequestTrackDownload(true, availability), true);
    assert.equal(canRequestTrackDownload(false, availability), false);
  }
  for (const availability of ["available", "fetching", undefined, null]) {
    assert.equal(canRequestTrackDownload(true, availability), false);
  }
});
