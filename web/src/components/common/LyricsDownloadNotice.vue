<template>
  <ModalDialog :isOpen="open" :closeCallback="close" closeOnEsc>
    <div class="lyricsNotice" role="status" aria-live="polite">
      <h2>Download lyrics</h2>
      <p>{{ message }}</p>
      <button type="button" @click="close">Close</button>
    </div>
  </ModalDialog>
</template>

<script setup>
import { ref } from "vue";
import ModalDialog from "@/components/common/ModalDialog.vue";
import { useRemoteStore } from "@/store/remote";

const remote = useRemoteStore();
const open = ref(false);
const message = ref("");
let inFlight = false;
const close = () => {
  open.value = false;
};
const download = async (entityType, id) => {
  if (!id || inFlight) return;
  inFlight = true;
  open.value = true;
  message.value = "Starting lyrics download…";
  try {
    const result = await remote.downloadLyrics(entityType, id);
    message.value = `Lyrics fetching started for ${result.tracks} ${result.tracks === 1 ? "track" : "tracks"}. Lyrics already downloaded will be reused.`;
  } catch (error) {
    message.value =
      error.response?.data?.message ||
      "Could not start the lyrics download. Please try again.";
  } finally {
    inFlight = false;
  }
};
defineExpose({ download });
</script>

<style scoped>
.lyricsNotice {
  max-width: 420px;
  padding: 24px;
}
.lyricsNotice p {
  margin: 16px 0;
}
.lyricsNotice button {
  padding: 8px 20px;
  cursor: pointer;
}
</style>
