import { defineStore } from 'pinia';
import { ref } from 'vue';
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
    console.log(`Loaded ${itemType} ${itemId} from storage`, item);
    if (item) {
      return JSON.parse(item);
    }
    return null;
  }

  const fetchItemFromRemote = async (itemType, itemId) => {
    let item = null;
    if (itemType === 'albums') {
      item = await remoteStore.fetchAlbum(itemId);
    } else if (itemType === 'artists') {
      item = await remoteStore.fetchArtist(itemId);
    } else if (itemType === 'tracks') {
      item = await remoteStore.fetchTrack(itemId);
    }

    console.log(`Fetched ${itemType} ${itemId}`, item);
    if (item) {
      localStorage.setItem(getStoredItemKey(itemType, itemId), JSON.stringify(item));
    }

    return item;
  }

  const triggerStaticItemFetch = (itemType, itemId) => {
    statics[itemType][itemId].ref.value.loading = true;
    let fetchedItem = loadFetchItemFromStorage(itemType, itemId);
    if (!fetchedItem) {
      fetchedItem = fetchItemFromRemote(itemType, itemId);
    }

    if (fetchedItem) {
      statics[itemType][itemId].ref.value.item = fetchedItem;
    } else {
      statics[itemType][itemId].ref.value.error = 'Failed to fetch item';
    }
    statics[itemType][itemId].ref.value.loading = false;
  }

  const getItem = (type, id) => {
    let entry = statics[type][id];
    if (entry) {
      console.log("getItem", type, id, "found entry", entry);
      if (!entry.item && !entry.loading) {
        triggerStaticItemFetch(type, id);
      }
      return entry.ref;
    }
    console.log("getItem", type, id, "entry not found returning default");

    entry = {
      id: id,
      ref: ref({
        loading: false,
        error: null,
        item: null,
      })
    };
    statics[type][id] = entry;
    triggerStaticItemFetch(type, id);
    return entry.ref;
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
  }
});
