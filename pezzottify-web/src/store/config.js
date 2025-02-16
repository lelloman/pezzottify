import { defineStore } from 'pinia';
import { ref, watch } from 'vue';

export const useConfigStore = defineStore('config', () => {

  const imagesEnabledValue = localStorage.getItem("imagesEnabled") === "false" ? false : true;
  const imagesEnabled = ref(imagesEnabledValue);

  watch(imagesEnabled, (v) => localStorage.setItem("imagesEnabled", v));
  return {
    imagesEnabled,
  };
});
