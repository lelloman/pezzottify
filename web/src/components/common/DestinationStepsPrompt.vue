<template>
  <ModalDialog :isOpen="isOpen" :closeCallback="close" :closeOnEsc="true">
    <form class="destinationPrompt" @submit.prevent="confirm">
      <header class="promptHeader">
        <SteeringArtwork
          v-if="reference"
          :reference="reference"
          size="md"
          showLabel
        />
        <div class="promptTitle">
          <span class="promptKicker">Heading to</span>
          <h2>{{ label || "Steer here" }}</h2>
        </div>
      </header>
      <p class="promptText">
        Smart continuation will steer the queue toward
        <strong>{{
          label || "this " + (reference?.entity_type || "item")
        }}</strong>
        over the next tracks it adds.
      </p>
      <p v-if="existingParts" class="promptText">
        This replaces the current destination mix ({{ existingParts }}
        {{ existingParts === 1 ? "part" : "parts" }}). Use "Add to playback
        destination" to extend it instead.
      </p>
      <p v-if="!canSteer" class="promptWarning" role="alert">
        {{ blockedReason }}
      </p>
      <div class="stepper">
        <span>Get there in</span>
        <span class="stepperControl">
          <button
            type="button"
            aria-label="Fewer tracks"
            :disabled="steps <= 1"
            @click="steps = Math.max(1, (Number(steps) || 1) - 1)"
          >
            −
          </button>
          <input
            ref="stepsInput"
            v-model.number="steps"
            type="number"
            min="1"
            max="500"
            required
            aria-label="Tracks to get there"
          />
          <button
            type="button"
            aria-label="More tracks"
            :disabled="steps >= 500"
            @click="steps = Math.min(500, (Number(steps) || 0) + 1)"
          >
            +
          </button>
        </span>
        <span>tracks</span>
      </div>
      <label v-if="!smartContinuationEnabled" class="checkRow">
        <input v-model="enableSmartContinuation" type="checkbox" />
        <span>Turn on smart continuation</span>
      </label>
      <footer class="promptActions">
        <button type="button" class="pill cancelButton" @click="close">
          Cancel
        </button>
        <button
          type="submit"
          class="pill primary"
          :disabled="!canSteer || !validSteps"
        >
          Steer here
        </button>
      </footer>
    </form>
  </ModalDialog>
</template>

<script setup>
import { computed, nextTick, ref, watch } from "vue";
import ModalDialog from "@/components/common/ModalDialog.vue";
import SteeringArtwork from "@/components/steering/SteeringArtwork.vue";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";
import { DEFAULT_STEPS_TOTAL } from "@/utils/gravity";

const props = defineProps({
  isOpen: {
    type: Boolean,
    required: true,
  },
  // { entity_type, entity_id, label? }
  reference: {
    type: Object,
    default: null,
  },
});

const emit = defineEmits(["close"]);

const playback = usePlaybackStore();
const userStore = useUserStore();

const steps = ref(DEFAULT_STEPS_TOTAL);
const enableSmartContinuation = ref(true);
const stepsInput = ref(null);

const label = computed(() => props.reference?.label || "");
const existingParts = computed(
  () => playback.currentGravity?.destination?.length || 0,
);
const smartContinuationEnabled = computed(
  () => userStore.isSmartContinuationEnabled,
);
const isRadio = computed(
  () => playback.currentPlaylist?.type === playback.PLAYBACK_CONTEXTS.radio,
);
const canSteer = computed(
  () => Boolean(playback.currentPlaylist) && !isRadio.value,
);
const blockedReason = computed(() =>
  isRadio.value
    ? "Radio queues continue from their own seed and cannot be steered."
    : "Start playing something first: steering applies to the current queue.",
);
const validSteps = computed(
  () => Number.isInteger(steps.value) && steps.value >= 1 && steps.value <= 500,
);

watch(
  () => props.isOpen,
  async (isOpen) => {
    if (!isOpen) return;
    steps.value = playback.currentGravity?.steps_total ?? DEFAULT_STEPS_TOTAL;
    enableSmartContinuation.value = true;
    await nextTick();
    stepsInput.value?.select();
  },
);

const close = () => emit("close");

const confirm = async () => {
  if (!canSteer.value || !validSteps.value || !props.reference) return;
  if (!smartContinuationEnabled.value && enableSmartContinuation.value) {
    userStore.setSmartContinuationEnabled(true);
  }
  await playback.setGravityDestination(props.reference, steps.value);
  close();
};
</script>

<style scoped>
@import "@/components/steering/steeringCard.css";

.destinationPrompt {
  width: min(440px, calc(100vw - 80px));
  display: flex;
  flex-direction: column;
  gap: 24px;
  color: var(--text-bright);
  color-scheme: dark;
}

.promptHeader {
  display: flex;
  align-items: center;
  gap: 16px;
}

.promptTitle {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.promptKicker {
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  letter-spacing: 0;
}

h2 {
  margin: 0;
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}

.promptText,
.promptWarning {
  margin: 0;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  line-height: 1.6;
}

.promptWarning {
  color: var(--warning-color, #f0b35b);
}

.checkRow {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

input[type="checkbox"] {
  accent-color: var(--spotify-green);
}

.promptActions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 4px;
}

.promptActions {
  padding-top: 8px;
  gap: 16px;
}
.promptActions .pill {
  min-height: 48px;
  padding-inline: 24px;
}
.cancelButton {
  border-color: transparent;
  color: var(--text-subdued);
}
.cancelButton:hover:not(:disabled) {
  border-color: transparent;
  color: var(--text-base);
}
.stepper {
  justify-content: space-between;
  padding: 16px 0;
  border-block: 1px solid var(--surface-border);
}
.stepperControl {
  border-radius: 6px;
  background: var(--bg-highlight);
}
.stepperControl button {
  width: 40px;
  height: 40px;
}
.promptText strong {
  color: var(--text-base);
  font-weight: 600;
}
</style>
