// Continuation belongs to the playback session, not to its display/listening context.
export function createRadioContinuation(trackIds, snapshot = null) {
  return {
    session_id:
      globalThis.crypto?.randomUUID?.() ??
      `radio-${Date.now()}-${Math.random().toString(36).slice(2)}`,
    strategy: snapshot ? "ranked_snapshot" : "seeded_radio",
    status:
      snapshot && trackIds.length >= snapshot.length ? "exhausted" : "active",
    seen_track_ids: [...new Set(trackIds)],
    ordered_track_ids: snapshot || [],
    next_index: snapshot ? trackIds.length : 0,
  };
}

export function editRadioContinuation(state, trackIds, keepGoing) {
  return {
    ...state,
    status: keepGoing ? state.status : "stopped",
    seen_track_ids: [...new Set([...state.seen_track_ids, ...trackIds])],
  };
}

export function nextSnapshotBatch(state, count = 10) {
  const seen = new Set(state.seen_track_ids);
  const trackIds = [];
  let nextIndex = state.next_index;
  while (
    nextIndex < state.ordered_track_ids.length &&
    trackIds.length < count
  ) {
    const id = state.ordered_track_ids[nextIndex++];
    if (!seen.has(id)) {
      seen.add(id);
      trackIds.push(id);
    }
  }
  return { trackIds, nextIndex };
}

export function appendRadioBatch(playlist, currentIndex, trackIds, nextIndex) {
  const state = playlist.continuation;
  const seen = new Set(state.seen_track_ids);
  const additions = [...new Set(trackIds)].filter((id) => !seen.has(id));
  const trim = Math.min(
    currentIndex,
    Math.max(0, playlist.tracksIds.length + additions.length - 500),
  );
  const cursor = nextIndex ?? state.next_index;
  return {
    index: currentIndex - trim,
    playlist: {
      ...playlist,
      tracksIds: [...playlist.tracksIds.slice(trim), ...additions],
      continuation: {
        ...state,
        next_index: cursor,
        seen_track_ids: [...seen, ...additions],
        status:
          !additions.length ||
          (state.strategy === "ranked_snapshot" &&
            cursor >= state.ordered_track_ids.length)
            ? "exhausted"
            : "active",
      },
    },
  };
}
