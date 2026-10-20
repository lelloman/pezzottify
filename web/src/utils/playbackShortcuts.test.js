import test from "node:test";
import assert from "node:assert/strict";
import { nextTick, ref } from "vue";
import {
  handlePlaybackShortcut,
  isEditableTarget,
  pausePlaybackDuringMaintenance,
} from "./playbackShortcuts.js";

function fakePlayback() {
  return {
    pauses: 0,
    toggles: 0,
    pause() {
      this.pauses++;
    },
    playPause() {
      this.toggles++;
    },
  };
}

test("entering maintenance pauses loaded playback and blocks the space shortcut", async () => {
  const maintenanceActive = ref(false);
  const playback = fakePlayback();
  const stop = pausePlaybackDuringMaintenance(maintenanceActive, playback);

  maintenanceActive.value = true;
  await nextTick();

  assert.equal(playback.pauses, 1);
  handlePlaybackShortcut(
    { key: " ", target: { tagName: "BODY" } },
    { isMaintenanceActive: () => maintenanceActive.value, playback },
  );
  assert.equal(playback.toggles, 0);
  stop();
});

test("the space shortcut still toggles playback outside maintenance", () => {
  const playback = fakePlayback();
  handlePlaybackShortcut(
    { key: " ", target: { tagName: "BODY" } },
    { isMaintenanceActive: () => false, playback },
  );
  assert.equal(playback.toggles, 1);
});

test("editable targets keep the space shortcut for typing", () => {
  const playback = fakePlayback();
  assert.equal(isEditableTarget({ tagName: "INPUT" }), true);
  assert.equal(isEditableTarget({ tagName: "DIV", isContentEditable: true }), true);
  handlePlaybackShortcut(
    { key: " ", target: { tagName: "TEXTAREA" } },
    { isMaintenanceActive: () => false, playback },
  );
  assert.equal(playback.toggles, 0);
});
