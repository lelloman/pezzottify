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

  const fetchItemFromRemote = (itemType, itemId) => {
    let itemPromise = null;
    if (itemType === 'albums') {
      itemPromise = remoteStore.fetchAlbum(itemId);
    } else if (itemType === 'artists') {
      itemPromise = remoteStore.fetchArtist(itemId);
    } else if (itemType === 'tracks') {
      itemPromise = remoteStore.fetchTrack(itemId);
    }
    console.log("staticsStore fetchItemFromRemote itemPromise", itemPromise);
    const item = Promise.resolve(itemPromise);
    console.log("staticsStore fetchItemFromRemote item", item);

    return item;
  }

  const triggerStaticItemFetch = (itemType, itemId) => {
    const storedItem = loadFetchItemFromStorage(itemType, itemId);
    if (storedItem) {
      statics[itemType][itemId].ref.item = storedItem;
      return;
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
