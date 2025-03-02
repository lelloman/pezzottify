import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

export const useConfigStore = defineStore('config', () => {

  const imagesEnabledValue = localStorage.getItem("imagesEnabled") === "false" ? false : true;
  const imagesEnabled = ref(imagesEnabledValue);

  const blockRightClickValue = localStorage.getItem("blockRightClick") === "false" ? false : true;
  const blockRightClick = ref(blockRightClickValue);

  const blockHttpCache = ref(localStorage.getItem("blockHttpCache") === "true");

  watch(imagesEnabled, (v) => localStorage.setItem("imagesEnabled", v));
  watch(blockHttpCache, (v) => localStorage.setItem("blockHttpCache", v));
  watch(blockRightClick, (v) => localStorage.setItem("blockRightClick", v));
  return {
    imagesEnabled, blockHttpCache, blockRightClick
  };
});
