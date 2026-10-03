<template>
  <ModalDialog :isOpen="isOpen" :closeCallback="close" :closeOnEsc="true">
    <form class="destinationPrompt" @submit.prevent="confirm">
      <h2>Set as playback destination</h2>
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
      <label>
        <span>Steps</span>
        <input
          ref="stepsInput"
          v-model.number="steps"
          type="number"
          min="1"
          max="500"
          required
        />
      </label>
      <label v-if="!smartContinuationEnabled" class="checkRow">
        <input v-model="enableSmartContinuation" type="checkbox" />
        <span>Turn on smart continuation</span>
      </label>
      <footer class="promptActions">
        <button type="button" @click="close">Cancel</button>
        <button
          type="submit"
          class="primaryButton"
          :disabled="!canSteer || !validSteps"
        >
          Steer
        </button>
      </footer>
    </form>
  </ModalDialog>
</template>

<script setup>
import { computed, nextTick, ref, watch } from "vue";
import ModalDialog from "@/components/common/ModalDialog.vue";
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
  () => Number.isInteger(steps.value) && steps.value >= 1,
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
.destinationPrompt {
  width: min(420px, 86vw);
  display: flex;
  flex-direction: column;
  gap: 14px;
  color: var(--text-bright);
  color-scheme: dark;
}

h2 {
  margin: 0;
}

.promptText,
.promptWarning {
  margin: 0;
  color: var(--text-subdued);
  line-height: 1.4;
}

.promptWarning {
  color: var(--warning-color, #f0b35b);
}

label {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

label span {
  color: var(--text-subdued);
}

.checkRow {
  flex-direction: row;
  align-items: center;
}

input[type="number"] {
  min-height: 34px;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
  color: var(--text-bright);
  padding: 6px 10px;
  outline: none;
}

input[type="number"]:focus {
  border-color: var(--spotify-green);
}

input[type="checkbox"] {
  accent-color: var(--spotify-green);
}

.promptActions {
  display: flex;
  justify-content: flex-end;
  gap: 12px;
}

button {
  min-height: 34px;
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
  color: var(--text-bright);
  padding: 0 12px;
}

button:hover:not(:disabled) {
  background: var(--bg-press);
}

button:disabled {
  cursor: default;
  opacity: 0.6;
}

.primaryButton {
  background: var(--accent-color);
  font-weight: var(--font-semibold);
}

.primaryButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}
</style>
