import { defineStore } from "pinia";
import { ref, watch } from "vue";

export const useDebugStore = defineStore("debug", () => {
  const imagesEnabledValue =
    localStorage.getItem("imagesEnabled") === "false" ? false : true;
  const imagesEnabled = ref(imagesEnabledValue);

  const blockRightClickValue =
    localStorage.getItem("blockRightClick") === "false" ? false : true;
  const blockRightClick = ref(blockRightClickValue);

  const blockHttpCache = ref(localStorage.getItem("blockHttpCache") === "true");

  // Organic search is disabled by default (smart/streaming search is the default)
  const useOrganicSearchValue =
    localStorage.getItem("useOrganicSearch") === "true" ? true : false;
  const useOrganicSearch = ref(useOrganicSearchValue);

  // Exclude unavailable content from search results (enabled by default)
  const excludeUnavailableValue =
    localStorage.getItem("excludeUnavailable") === "false" ? false : true;
  const excludeUnavailable = ref(excludeUnavailableValue);

  watch(imagesEnabled, (v) => localStorage.setItem("imagesEnabled", v));
  watch(blockHttpCache, (v) => localStorage.setItem("blockHttpCache", v));
  watch(blockRightClick, (v) => localStorage.setItem("blockRightClick", v));
  watch(useOrganicSearch, (v) => localStorage.setItem("useOrganicSearch", v));
  watch(excludeUnavailable, (v) => localStorage.setItem("excludeUnavailable", v));

  const clearLocalStorageStatics = () => {
    // Removes all items that start with "statics_" from localStorage
    const keysToRemove = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key.startsWith("statics_")) {
        keysToRemove.push(key);
      }
    }
    keysToRemove.forEach((key) => localStorage.removeItem(key));
  };
  return {
    imagesEnabled,
    blockHttpCache,
    blockRightClick,
    useOrganicSearch,
    excludeUnavailable,
    clearLocalStorageStatics,
  };
});
