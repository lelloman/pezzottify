import { defineStore } from "pinia";
import { reactive } from "vue";
import { useRemoteStore } from "./remote";

export const useStaticsStore = defineStore("statics", () => {
  const remoteStore = useRemoteStore();

  const statics = {
    albums: {},
    artists: {},
    tracks: {},
  };

  const getStoredItemKey = (itemType, itemId) => {
    return `statics_${itemType}_${itemId}`;
  };

  const loadFetchItemFromStorage = (itemType, itemId) => {
    try {
      const item = localStorage.getItem(getStoredItemKey(itemType, itemId));
      return item ? JSON.parse(item) : null;
    } catch {
      // Corrupt or unavailable browser storage must not prevent a remote load.
      return null;
    }
  };

  // Transform ResolvedArtist response to legacy format
  const transformArtistResponse = (resolvedArtist) => {
    if (!resolvedArtist) return null;

    // If it's already in the old format (has 'name' at top level), return as-is
    if (resolvedArtist.name) return resolvedArtist;

    // Transform ResolvedArtist to legacy format
    const artist = {
      ...resolvedArtist.artist,
      portrait_group: resolvedArtist.display_image
        ? [resolvedArtist.display_image]
        : [],
      portraits: [],
      related: resolvedArtist.related_artists
        ? resolvedArtist.related_artists.map((a) => a.id)
        : [],
      enrichment_status: resolvedArtist.enrichment_status || null,
      enrichment: resolvedArtist.enrichment || null,
    };

    return artist;
  };

  // Transform ResolvedAlbum response to legacy format
  const transformAlbumResponse = (resolvedAlbum) => {
    if (!resolvedAlbum) return null;

    // If it's already in the old format (has 'name' at top level), return as-is
    if (resolvedAlbum.name) return resolvedAlbum;

    // Transform discs: convert track objects to track IDs
    const discs = resolvedAlbum.discs
      ? resolvedAlbum.discs.map((disc) => ({
          name: disc.name,
          number: disc.number,
          tracks: disc.tracks.map((t) => t.id),
        }))
      : [];

    // Transform ResolvedAlbum to legacy format
    const album = {
      ...resolvedAlbum.album,
      covers: resolvedAlbum.display_image ? [resolvedAlbum.display_image] : [],
      cover_group: [],
      artists_ids: resolvedAlbum.artists
        ? resolvedAlbum.artists.map((a) => a.id)
        : [],
      discs: discs,
      enrichment_status: resolvedAlbum.enrichment_status || null,
      enrichment: resolvedAlbum.enrichment || null,
    };

    return album;
  };

  // Transform ResolvedTrack response to legacy format
  const transformTrackResponse = (resolvedTrack) => {
    if (!resolvedTrack) return null;

    // If it's already in the old format (has 'artists_ids' at top level), return as-is
    if (resolvedTrack.artists_ids) return resolvedTrack;

    // Transform ResolvedTrack to legacy format
    const track = {
      ...resolvedTrack.track,
      artists_ids: resolvedTrack.artists
        ? resolvedTrack.artists.map((a) => a.artist.id)
        : [],
      // Duration in ms (server field is duration_ms)
      duration: resolvedTrack.track.duration_ms || null,
      // Include availability state (defaults to available if not present)
      availability: resolvedTrack.track.availability || "available",
      enrichment_status: resolvedTrack.enrichment_status || null,
      enrichment: resolvedTrack.enrichment || null,
      work_resolution: resolvedTrack.work_resolution || null,
      work_enrichment_status: resolvedTrack.work_enrichment_status || null,
    };

    return track;
  };

  const fetchItemFromRemote = (itemType, itemId) => {
    let itemPromise = null;
    if (itemType === "albums") {
      // Use fetchResolvedAlbum to get display_image, artists, and tracks
      itemPromise = remoteStore
        .fetchResolvedAlbum(itemId)
        .then(transformAlbumResponse);
    } else if (itemType === "artists") {
      itemPromise = remoteStore
        .fetchArtist(itemId)
        .then(transformArtistResponse);
    } else if (itemType === "tracks") {
      // Use fetchResolvedTrack to get artists info
      itemPromise = remoteStore
        .fetchResolvedTrack(itemId)
        .then(transformTrackResponse);
    }

    return Promise.resolve(itemPromise);
  };

  // Validate cached item has all required fields
  const isValidCachedItem = (itemType, item) => {
    if (!item) return false;
    // Tracks must have artists_ids (may be missing from old cache)
    if (itemType === "tracks" && !item.artists_ids) return false;
    // Albums must have covers array (may be missing from old cache before display_image transform)
    // We check for the array existence, not length - albums may genuinely have no images
    // but the array should exist after the transform
    if (itemType === "albums" && !Array.isArray(item.covers)) return false;
    // Artists must have portrait_group array (may be missing from old cache before display_image transform)
    if (itemType === "artists" && !Array.isArray(item.portrait_group))
      return false;
    return true;
  };

  const updateItemFromRemote = (itemType, itemId) => {
    const entry = statics[itemType][itemId];
    if (entry.promise) return entry.promise;

    // Install the shared promise before doing any reactive writes. Getters can
    // run repeatedly during rendering without starting duplicate requests.
    entry.promise = Promise.resolve().then(async () => {
      try {
        for (;;) {
          const revision = entry.revision;
          let fetchedItem;
          try {
            fetchedItem = await fetchItemFromRemote(itemType, itemId);
            if (!fetchedItem) throw new Error("Failed to fetch item");
          } catch {
            if (revision !== entry.revision) continue;
            if (!entry.ref.item) entry.ref.error = "Failed to fetch item";
            return null;
          }
          // A catalog invalidation during the request requires one fresh load.
          // Discard the old response without publishing it or caching it.
          if (revision !== entry.revision) continue;
          try {
            localStorage.setItem(
              getStoredItemKey(itemType, itemId),
              JSON.stringify(fetchedItem),
            );
          } catch {
            // Quota/privacy failures do not discard successfully loaded data.
          }
          entry.ref.error = null;
          entry.ref.item = fetchedItem;
          return fetchedItem;
        }
      } finally {
        entry.promise = null;
      }
    });
    return entry.promise;
  };

  const getItem = (type, id) => {
    let entry = statics[type][id];
    // Loading and failed entries are both stable. Explicit invalidation, rather
    // than a render/computed getter, starts another attempt after a failure.
    if (entry) return entry.ref;

    const storedItem = loadFetchItemFromStorage(type, id);
    entry = {
      ref: reactive({
        error: null,
        item: isValidCachedItem(type, storedItem) ? storedItem : null,
      }),
      promise: null,
      revision: 0,
    };
    statics[type][id] = entry;
    // Serve valid cached data immediately and refresh once in the background.
    void updateItemFromRemote(type, id);
    return entry.ref;
  };

  const getItemData = (type, id) => statics[type][id]?.ref.item ?? null;

  const waitItemData = (type, id) => {
    const itemRef = getItem(type, id);
    if (itemRef.item) return Promise.resolve(itemRef.item);
    const entry = statics[type][id];
    if (entry.promise) {
      return entry.promise.then((item) => {
        if (!item) throw new Error(itemRef.error || "Failed to fetch item");
        return item;
      });
    }
    return Promise.reject(new Error(itemRef.error || "Failed to fetch item"));
  };

  const waitAlbumData = (albumId) => {
    return waitItemData("albums", albumId);
  };

  const waitArtistData = (artistId) => {
    return waitItemData("artists", artistId);
  };

  const waitTrackData = (trackId) => {
    return waitItemData("tracks", trackId);
  };

  const getAlbumData = (albumId) => {
    return getItemData("albums", albumId);
  };
  const getTrackData = (trackId) => {
    return getItemData("tracks", trackId);
  };

  const getAlbum = (albumId) => {
    return getItem("albums", albumId);
  };

  const getArtist = (artistId) => {
    return getItem("artists", artistId);
  };

  const getTrack = (trackId) => {
    return getItem("tracks", trackId);
  };

  // Invalidate a cached item (remove from memory and localStorage)
  const invalidateItem = (itemType, itemId) => {
    try {
      localStorage.removeItem(getStoredItemKey(itemType, itemId));
    } catch {
      // In-memory invalidation still works without browser storage.
    }
    const entry = statics[itemType]?.[itemId];
    if (!entry) return;
    entry.revision++;
    entry.ref.error = null;
    entry.ref.item = null;
    // Keep the same reactive reference so mounted rows receive the refresh.
    void updateItemFromRemote(itemType, itemId);
  };

  return {
    getAlbum,
    getArtist,
    getTrack,
    getAlbumData,
    getTrackData,
    waitAlbumData,
    waitArtistData,
    waitTrackData,
    invalidateItem,
  };
});
