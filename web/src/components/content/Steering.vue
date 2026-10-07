<template>
  <div class="steeringPage">
    <header class="hero" :style="heroStyle">
      <div class="heroText">
        <span class="eyebrow">Smart continuation</span>
        <h1 class="heroTitle">Steering</h1>
        <p class="heroIntro">
          Smart continuation picks what plays next. Steer it from where your
          queue started toward somewhere new.
        </p>
      </div>

      <div v-if="gravity" class="journey">
        <div class="journeyEnd">
          <div class="journeyArt">
            <QueueCollage
              v-if="gravity.source.kind === 'queue'"
              :tracksIds="chosenTracksIds"
            />
            <SteeringArtwork
              v-else-if="sourceReferences.length === 1"
              :reference="sourceReferences[0]"
              size="lg"
              showLabel
            />
            <div v-else class="mixArt">
              <SteeringArtwork
                v-for="reference in sourceReferences.slice(0, 4)"
                :key="referenceKey(reference)"
                :reference="reference"
                size="lg"
              />
            </div>
          </div>
          <span class="journeyKicker">Starting point</span>
          <span class="journeyName">{{ sourceTitle }}</span>
        </div>

        <div class="journeyPath">
          <template v-if="destination.length">
            <div
              class="pathTrack"
              role="progressbar"
              aria-label="Steering progress"
              :aria-valuenow="progressPercent"
              aria-valuemin="0"
              aria-valuemax="100"
            >
              <div class="pathFill" :style="{ width: progressPercent + '%' }" />
              <span class="pathDot" :style="{ left: progressPercent + '%' }" />
            </div>
            <span class="pathText">
              {{ gravity.steps_done }} of {{ gravity.steps_total }} tracks along
              the way
            </span>
          </template>
          <template v-else>
            <div class="pathTrack idle" aria-hidden="true" />
            <button
              type="button"
              class="pill primary large pathButton"
              data-test="open-picker"
              @click="openDestinationPicker"
            >
              Pick where to head
            </button>
          </template>
        </div>

        <div class="journeyEnd">
          <div class="journeyArt">
            <SteeringArtwork
              v-if="destination.length === 1"
              :reference="destination[0]"
              size="lg"
              showLabel
            />
            <div v-else-if="destination.length" class="mixArt">
              <SteeringArtwork
                v-for="reference in destination.slice(0, 4)"
                :key="referenceKey(reference)"
                :reference="reference"
                size="lg"
              />
            </div>
            <button
              v-else
              type="button"
              class="destinationPlaceholder"
              aria-label="Pick where to head"
              @click="openDestinationPicker"
            >
              +
            </button>
          </div>
          <span class="journeyKicker">Heading to</span>
          <span class="journeyName">
            {{ destination.length ? mixTitle(destination) : "Nowhere yet" }}
          </span>
        </div>
      </div>
    </header>

    <div class="steeringContent">
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
          <GravityDestinationCard ref="destinationCard" :gravity="gravity" />
        </div>
        <GravityJourneyCard :gravity="gravity" />
      </template>
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, ref } from "vue";
import GravitySourceCard from "@/components/steering/GravitySourceCard.vue";
import GravityDestinationCard from "@/components/steering/GravityDestinationCard.vue";
import GravityJourneyCard from "@/components/steering/GravityJourneyCard.vue";
import SteeringArtwork from "@/components/steering/SteeringArtwork.vue";
import QueueCollage from "@/components/steering/QueueCollage.vue";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";
import { progress } from "@/utils/gravity";
import {
  mixTitle,
  referenceKey,
  tileColor,
  withAlpha,
} from "@/utils/steeringArt";

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

const destination = computed(() => gravity.value?.destination || []);
const sourceReferences = computed(() =>
  gravity.value?.source?.kind === "references"
    ? gravity.value.source.references || []
    : [],
);
const chosenTracksIds = computed(() => {
  const auto = new Set(gravity.value?.auto_track_ids || []);
  return tracksIds.value.filter((id) => !auto.has(id));
});
const sourceTitle = computed(() =>
  sourceReferences.value.length
    ? mixTitle(sourceReferences.value)
    : "Your queue",
);
const progressPercent = computed(() =>
  gravity.value ? Math.round(progress(gravity.value) * 100) : 0,
);

// Tinted by where the queue is heading; neutral while not steering.
const heroStyle = computed(() => {
  const colour = destination.value.length
    ? tileColor(destination.value[0].entity_id)
    : "#4a5055";
  return {
    background: `linear-gradient(180deg, ${withAlpha(colour, 0.85)} 0%, ${withAlpha(colour, 0.35)} 55%, rgba(0, 0, 0, 0) 100%)`,
  };
});

const destinationCard = ref(null);
const openDestinationPicker = async () => {
  destinationCard.value?.openPicker();
  await nextTick();
  destinationCard.value?.$el?.scrollIntoView?.({
    behavior: "smooth",
    block: "start",
  });
};
</script>

<style scoped>
@import "@/components/steering/steeringCard.css";

.steeringPage {
  display: flex;
  flex-direction: column;
  gap: 24px;
  width: 100%;
  min-height: 100%;
  color: var(--text-base);
}

.hero {
  display: flex;
  flex-direction: column;
  gap: 28px;
  padding: 32px 32px 28px;
  border-radius: 0;
  transition: background var(--transition-slow);
}

.heroText {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.eyebrow {
  color: var(--text-bright);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  letter-spacing: 0;
}

.heroTitle {
  margin: 0;
  color: var(--text-bright);
  font-size: clamp(2.5rem, 6cqw, 4.5rem);
  font-weight: var(--font-black);
  line-height: 1;
  letter-spacing: -0.04em;
}

.heroIntro {
  max-width: 56ch;
  margin: 6px 0 0;
  color: rgba(255, 255, 255, 0.78);
  font-size: var(--text-md);
  line-height: 1.45;
}

.journey {
  display: grid;
  grid-template-columns: minmax(0, 180px) minmax(0, 1fr) minmax(0, 180px);
  align-items: start;
  gap: 24px;
  max-width: 1040px;
}

.journeyEnd {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.journeyArt {
  width: 100%;
  aspect-ratio: 1;
  margin-bottom: 8px;
}

.journeyArt :deep(.steeringArtwork.size-lg) {
  box-shadow: var(--shadow-xl);
}

.mixArt {
  display: grid;
  grid-template-columns: 1fr 1fr;
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
  border-radius: 8px;
  box-shadow: var(--shadow-xl);
}

.mixArt {
  grid-template-rows: 1fr 1fr;
}

.mixArt :deep(.steeringArtwork) {
  width: 100%;
  height: 100%;
  aspect-ratio: auto;
  border-radius: 0;
  box-shadow: none;
}

/* Two parts side by side; with three, the first takes the left half. */
.mixArt :deep(.steeringArtwork:first-child:nth-last-child(2)),
.mixArt
  :deep(.steeringArtwork:first-child:nth-last-child(2) ~ .steeringArtwork),
.mixArt :deep(.steeringArtwork:first-child:nth-last-child(3)) {
  grid-row: span 2;
}

.destinationPlaceholder {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  border: 1px solid var(--surface-border-strong);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: rgba(255, 255, 255, 0.6);
  font-size: 3rem;
  font-weight: var(--font-normal);
  cursor: pointer;
  transition:
    border-color var(--transition-fast),
    color var(--transition-fast);
}

.destinationPlaceholder:hover {
  border-color: var(--text-bright);
  color: var(--text-bright);
}

.journeyKicker {
  color: rgba(255, 255, 255, 0.7);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  letter-spacing: 0;
}

.journeyName {
  overflow: hidden;
  color: var(--text-bright);
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  line-height: 1.25;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

/* The path line sits at the vertical middle of the artwork (180px / 96px). */
.journeyPath {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 14px;
  padding-top: 88px;
}

.pathTrack {
  position: relative;
  width: 100%;
  height: 4px;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.25);
}

.pathTrack.idle {
  background: repeating-linear-gradient(
    to right,
    rgba(255, 255, 255, 0.35) 0 8px,
    transparent 8px 16px
  );
}

.pathFill {
  position: absolute;
  inset: 0 auto 0 0;
  border-radius: inherit;
  background: var(--spotify-green);
}

.pathDot {
  position: absolute;
  top: 50%;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.5);
  transform: translate(-50%, -50%);
}

.pathText {
  color: var(--text-bright);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-align: center;
}

.pathButton {
  white-space: nowrap;
}

.emptyState {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 140px;
  padding: 16px;
  border-radius: var(--radius-xl);
  background: rgba(255, 255, 255, 0.035);
  color: var(--text-subdued);
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  text-align: center;
}

.notice {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 20px;
  border-radius: var(--radius-xl);
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-bright);
  font-size: var(--text-sm);
}

.noticeButton {
  min-height: 34px;
  padding: 0 18px;
  border: none;
  border-radius: var(--radius-full);
  background: var(--spotify-green);
  color: #000;
  font-weight: var(--font-bold);
  cursor: pointer;
}

.noticeButton:hover {
  background: var(--spotify-green-hover);
}

.steeringGrid {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
  gap: 24px;
  align-items: start;
}

@container (max-width: 850px) {
  .steeringGrid {
    grid-template-columns: minmax(0, 1fr);
  }
}

@container (max-width: 560px) {
  .hero {
    gap: 22px;
    padding: 24px 16px 20px;
  }

  .journey {
    grid-template-columns: minmax(0, 96px) minmax(0, 1fr) minmax(0, 96px);
    gap: 12px;
  }

  .journeyPath {
    padding-top: 46px;
  }

  .journeyName {
    font-size: var(--text-sm);
  }

  /* Too narrow to sit on the path; the "+" tile and the Heading to card's
     button open the same picker. */
  .pathButton {
    display: none;
  }

  .destinationPlaceholder {
    font-size: 2rem;
  }
}

.steeringContent {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 0 32px 40px;
}
.steeringGrid {
  gap: 16px;
}
.heroIntro {
  font-size: 16px;
  line-height: 1.6;
}
@container (max-width:560px) {
  .steeringContent {
    padding: 0 16px 24px;
    gap: 16px;
  }
}
</style>
