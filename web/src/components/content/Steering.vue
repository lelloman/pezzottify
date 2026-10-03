<template>
  <div class="steeringPage">
    <header class="steeringHeader">
      <h1 class="pageTitle">Steering</h1>
      <p class="pageIntro">
        Smart continuation picks what plays next. Steer it from where your queue
        started toward somewhere new.
      </p>
    </header>

    <div v-if="!playlist" class="emptyState">
      Nothing is playing. Start an album, playlist or track to steer its
      continuation.
    </div>

    <div v-else-if="isRadio" class="emptyState">
      Radio queues continue from their own seed and cannot be steered.
    </div>

    <template v-else-if="gravity">
      <div v-if="!smartContinuationEnabled" class="notice" role="status">
        <span>
          Smart continuation is off, so nothing will be added to the queue.
        </span>
        <button type="button" class="noticeButton" @click="enableSmart">
          Turn it on
        </button>
      </div>
      <div v-if="isRemote" class="notice" role="status">
        <span>
          You are controlling another device. Changes are sent to it.
        </span>
      </div>

      <div class="steeringGrid">
        <GravitySourceCard :gravity="gravity" :tracksIds="tracksIds" />
        <GravityDestinationCard :gravity="gravity" />
      </div>
      <GravityJourneyCard :gravity="gravity" />
    </template>
  </div>
</template>

<script setup>
import { computed } from "vue";
import GravitySourceCard from "@/components/steering/GravitySourceCard.vue";
import GravityDestinationCard from "@/components/steering/GravityDestinationCard.vue";
import GravityJourneyCard from "@/components/steering/GravityJourneyCard.vue";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";

const playback = usePlaybackStore();
const userStore = useUserStore();

const playlist = computed(() => playback.currentPlaylist);
const isRadio = computed(
  () => playlist.value?.type === playback.PLAYBACK_CONTEXTS.radio,
);
const gravity = computed(() => playback.currentGravity);
const tracksIds = computed(() => playlist.value?.tracksIds || []);
const isRemote = computed(() => playback.mode === "remote");
const smartContinuationEnabled = computed(
  () => userStore.isSmartContinuationEnabled,
);

const enableSmart = () => userStore.setSmartContinuationEnabled(true);
</script>

<style scoped>
.steeringPage {
  display: flex;
  flex-direction: column;
  gap: 20px;
  width: 100%;
  min-height: 100%;
  color: var(--text-base);
}

.steeringHeader {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.pageTitle {
  margin: 0;
  color: #9eddb7;
  font-size: clamp(1.25rem, 1.8vw, 1.65rem);
  font-weight: 900;
  line-height: 1.1;
  text-transform: uppercase;
}

.pageIntro {
  max-width: 68ch;
  margin: 0;
  color: var(--text-subdued);
  line-height: 1.45;
}

.emptyState {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 140px;
  padding: 16px;
  border: 1px dashed var(--surface-border);
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 0.9rem;
  font-weight: 700;
  text-align: center;
}

.notice {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--bg-highlight);
  color: var(--text-bright);
}

.noticeButton {
  min-height: 32px;
  border-radius: var(--radius-md);
  background: var(--accent-color);
  color: var(--text-bright);
  font-weight: var(--font-semibold);
  padding: 0 12px;
}

.noticeButton:hover {
  background: var(--spotify-green-hover);
}

.steeringGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
  gap: 16px;
  align-items: start;
}
</style>
