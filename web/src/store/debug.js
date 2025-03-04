import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

export const useDebugStore = defineStore('debug', () => {

  const imagesEnabledValue = localStorage.getItem("imagesEnabled") === "false" ? false : true;
  const imagesEnabled = ref(imagesEnabledValue);

  const blockRightClickValue = localStorage.getItem("blockRightClick") === "false" ? false : true;
  const blockRightClick = ref(blockRightClickValue);

  const blockHttpCache = ref(localStorage.getItem("blockHttpCache") === "true");

  watch(imagesEnabled, (v) => localStorage.setItem("imagesEnabled", v));
  watch(blockHttpCache, (v) => localStorage.setItem("blockHttpCache", v));
  watch(blockRightClick, (v) => localStorage.setItem("blockRightClick", v));

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
  }
  return {
    imagesEnabled, blockHttpCache, blockRightClick, clearLocalStorageStatics,
  };
});
