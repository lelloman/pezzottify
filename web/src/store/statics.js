import { defineStore } from 'pinia';
import { reactive, ref } from 'vue';
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
    getTrackData,
  }
});
