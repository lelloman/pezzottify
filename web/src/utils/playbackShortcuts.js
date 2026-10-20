import { watch } from "vue";

export function isEditableTarget(target) {
  return (
    target?.tagName === "INPUT" ||
    target?.tagName === "TEXTAREA" ||
    target?.isContentEditable === true
  );
}

export function handlePlaybackShortcut(
  event,
  { isMaintenanceActive, playback },
) {
  if (isMaintenanceActive()) return;
  if (event.key !== " " || isEditableTarget(event.target)) return;
  playback.playPause();
}

export function pausePlaybackDuringMaintenance(isMaintenanceActive, playback) {
  return watch(isMaintenanceActive, (active) => {
    if (active) playback.pause();
  });
}
