import { defineStore } from 'pinia';
import { toRefs, reactive } from 'vue';

export const usePlayerStore = defineStore('player', () => {
  const state = reactive({
    playlist: [],
    currentTrack: null,
    isPlaying: false,
  })

  const setTrack = (index) => {
    state.currentTrack.value = state.playlist.value[index];
  };

  const playPause = () => {
    state.isPlaying.value = !state.isPlaying.value;
  };

  return {
    ...toRefs(state),
    setTrack,
    playPause,
  };
});
