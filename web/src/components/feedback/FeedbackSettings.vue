<script setup>
import { useFeedbackStore } from "../../store/feedback";
const feedback = useFeedbackStore();
</script>
<template>
  <section>
    <h2>Feedback and assistant diagnostics</h2>
    <p>
      Recording is off by default. When enabled, future assistant activity is
      saved locally, up to 2 MiB. Browser storage may be unavailable or evicted.
      Nothing is uploaded automatically.
    </p>
    <label
      ><input
        type="checkbox"
        :checked="feedback.enabled"
        @change="feedback.setEnabled($event.target.checked)"
      />
      Record assistant diagnostics for this account</label
    >
    <p>
      Disabling recording, clearing chat, signing out or changing accounts
      deletes local diagnostics and unsent previews. Submitted reports remain on
      the server.
    </p>
    <p v-if="feedback.storageError" role="alert">{{ feedback.storageError }}</p>
    <button @click="feedback.clear()">Clear local diagnostics</button>
    <button @click="feedback.show()">Report a bug or request a feature</button>
  </section>
</template>
