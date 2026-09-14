// Download requests depend on stored content, not proxy playback availability.
export function canRequestAlbumDownload(hasPermission, availability) {
  return hasPermission && ["missing", "partial"].includes(availability);
}

export function canRequestTrackDownload(hasPermission, availability) {
  return (
    hasPermission &&
    Boolean(availability) &&
    availability !== "available" &&
    availability !== "fetching"
  );
}
