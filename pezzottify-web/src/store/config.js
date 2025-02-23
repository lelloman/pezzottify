import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

export const useConfigStore = defineStore('config', () => {

  const imagesEnabledValue = localStorage.getItem("imagesEnabled") === "false" ? false : true;
  const imagesEnabled = ref(imagesEnabledValue);

  const blockHttpCache = ref(localStorage.getItem("blockHttpCache") === "true");

  watch(imagesEnabled, (v) => localStorage.setItem("imagesEnabled", v));
  watch(blockHttpCache, (v) => localStorage.setItem("blockHttpCache", v));
  return {
    imagesEnabled, blockHttpCache,
  };
});
