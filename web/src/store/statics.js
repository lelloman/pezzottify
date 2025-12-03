import { defineStore } from 'pinia';
import { reactive, ref, watch } from 'vue';
import { useRemoteStore } from './remote';


export const useStaticsStore = defineStore('statics', () => {

  const remoteStore = useRemoteStore();

  const statics = {
    albums: {},
    artists: {},
    tracks: {},
  }

  const getStoredItemKey = (itemType, itemId) => {
    return `statics_${itemType}_${itemId}`;
  }

  const loadFetchItemFromStorage = (itemType, itemId) => {
    const item = localStorage.getItem(getStoredItemKey(itemType, itemId));
    if (item) {
      return JSON.parse(item);
    }
    return null;
  }

  // Transform ResolvedArtist response to legacy format
  const transformArtistResponse = (resolvedArtist) => {
    if (!resolvedArtist) return null;

    // If it's already in the old format (has 'name' at top level), return as-is
    if (resolvedArtist.name) return resolvedArtist;

    // Transform ResolvedArtist to legacy format
    const artist = {
      ...resolvedArtist.artist,
      portrait_group: resolvedArtist.display_image ? [resolvedArtist.display_image] : [],
      portraits: [],
      related: resolvedArtist.related_artists ? resolvedArtist.related_artists.map(a => a.id) : []
    };

    return artist;
  }

  // Transform ResolvedAlbum response to legacy format
  const transformAlbumResponse = (resolvedAlbum) => {
    if (!resolvedAlbum) return null;

    // If it's already in the old format (has 'name' at top level), return as-is
    if (resolvedAlbum.name) return resolvedAlbum;

    // Transform discs: convert track objects to track IDs
    const discs = resolvedAlbum.discs ? resolvedAlbum.discs.map(disc => ({
      name: disc.name,
      number: disc.number,
      tracks: disc.tracks.map(t => t.id)
    })) : [];

    // Transform ResolvedAlbum to legacy format
    const album = {
      ...resolvedAlbum.album,
      covers: resolvedAlbum.display_image ? [resolvedAlbum.display_image] : [],
      cover_group: [],
      artists_ids: resolvedAlbum.artists ? resolvedAlbum.artists.map(a => a.id) : [],
      discs: discs
    };

    return album;
  }

  // Transform ResolvedTrack response to legacy format
  const transformTrackResponse = (resolvedTrack) => {
    if (!resolvedTrack) return null;

    // If it's already in the old format (has 'artists_ids' at top level), return as-is
    if (resolvedTrack.artists_ids) return resolvedTrack;

    // Transform ResolvedTrack to legacy format
    const track = {
      ...resolvedTrack.track,
      artists_ids: resolvedTrack.artists ? resolvedTrack.artists.map(a => a.artist.id) : [],
      // Also include duration in ms for compatibility (server returns duration_secs)
      duration: resolvedTrack.track.duration_secs ? resolvedTrack.track.duration_secs * 1000 : null,
    };

    return track;
  }

  const fetchItemFromRemote = (itemType, itemId) => {
    let itemPromise = null;
    if (itemType === 'albums') {
      // Use fetchResolvedAlbum to get display_image, artists, and tracks
      itemPromise = remoteStore.fetchResolvedAlbum(itemId).then(transformAlbumResponse);
    } else if (itemType === 'artists') {
      itemPromise = remoteStore.fetchArtist(itemId).then(transformArtistResponse);
    } else if (itemType === 'tracks') {
      // Use fetchResolvedTrack to get artists info
      itemPromise = remoteStore.fetchResolvedTrack(itemId).then(transformTrackResponse);
    }

    return Promise.resolve(itemPromise);
  }

  // Validate cached item has all required fields
  const isValidCachedItem = (itemType, item) => {
    if (!item) return false;
    // Tracks must have artists_ids (may be missing from old cache)
    if (itemType === 'tracks' && !item.artists_ids) return false;
    return true;
  }

  const triggerStaticItemFetch = (itemType, itemId) => {
    const storedItem = loadFetchItemFromStorage(itemType, itemId);
    if (storedItem && isValidCachedItem(itemType, storedItem)) {
      statics[itemType][itemId].ref.item = storedItem;
      return;
    }
    // If cached item is invalid, remove it
    if (storedItem) {
      localStorage.removeItem(getStoredItemKey(itemType, itemId));
    }
    fetchItemFromRemote(itemType, itemId)
      .then((fetchedItem) => {
        localStorage.setItem(getStoredItemKey(itemType, itemId), JSON.stringify(fetchedItem));
        statics[itemType][itemId].ref.item = fetchedItem;
      })
      .catch((e) => {
        console.log("triggerStaticsItemFetch error:", e);
        statics[itemType][itemId].ref.error = 'Failed to fetch item';
      });
  }

  const getItem = (type, id) => {
    let entry = statics[type][id];
    if (entry) {
      if (!entry.item) {
        triggerStaticItemFetch(type, id);
      }
      return entry.ref;
    }

    entry = {
      id: id,
      ref: reactive({
        error: null,
        item: null,
      })
    };
    statics[type][id] = entry;
    triggerStaticItemFetch(type, id);
    return entry.ref;
  }

  const getItemData = (type, id) => {
    let entry = statics[type][id];
    if (entry) {
      return entry.ref.item;
    }
    return null;
  }

  // If the item is not present, it waits for it to be fetched
  const waitItemData = (type, id) => {
    let entry = statics[type][id];
    if (entry && entry.ref.item) {
      return Promise.resolve(entry.ref.item);
    }

    if (entry && !entry.ref.item) {
      return new Promise((resolve, reject) => {
        const stopWatchItem = watch(() => entry.ref.item, (newValue) => {
          if (newValue) {
            stopWatchItem();
            stopWatchError();
            resolve(newValue);
          }
        });

        const stopWatchError = watch(() => entry.ref.error, (newValue) => {
          if (newValue) {
            stopWatchItem();
            stopWatchError();
            reject(newValue);
          }
        });
      });
    }

    entry = {
      id: id,
      ref: reactive({
        error: null,
        item: null,
      })
    };
    statics[type][id] = entry;
    return new Promise((resolve, reject) => {
      fetchItemFromRemote(type, id)
        .then((item) => {
          localStorage.setItem(getStoredItemKey(type, id), JSON.stringify(item));
          entry.ref.item = item;
          resolve(item);
        })
        .catch((e) => {
          entry.ref.error = 'Failed to fetch item';
          reject(e);
        });
    });
  }

  const waitAlbumData = (albumId) => {
    return waitItemData('albums', albumId);
  }

  const getAlbumData = (albumId) => {
    return getItemData('albums', albumId);
  }
  const getTrackData = (trackId) => {
    return getItemData('tracks', trackId);
  }

  const getAlbum = (albumId) => {
    return getItem('albums', albumId);
  }

  const getArtist = (artistId) => {
    return getItem('artists', artistId);
  }

  const getTrack = (trackId) => {
    return getItem('tracks', trackId);
  }

  return {
    getAlbum,
    getArtist,
    getTrack,
    getAlbumData,
    getTrackData,
    waitAlbumData,
  }
});
