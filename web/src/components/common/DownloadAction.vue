<template>
  <div
    v-if="state !== 'completed'"
    class="downloadAction"
    :class="{ failed, active: state === 'in_progress' }"
  >
    <button
      v-if="state === 'can_request' || failed"
      type="button"
      class="requestButton"
      :disabled="busy"
      @click="$emit('request')"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 3v12m-5-5 5 5 5-5M5 16v4h14v-4" />
      </svg>
      {{
        busy ? "Requesting…" : failed ? "Retry download" : "Request download"
      }}
    </button>
    <span
      v-if="state !== 'can_request'"
      class="downloadStatus"
      role="status"
      aria-live="polite"
      :title="error || undefined"
    >
      <svg v-if="!failed" viewBox="0 0 24 24" aria-hidden="true">
        <template v-if="state === 'pending'">
          <circle cx="12" cy="12" r="9" />
          <path d="M12 7v5l3 2" />
        </template>
        <template v-else><path d="M12 3v12m-5-5 5 5 5-5M5 19h14" /></template>
      </svg>
      <span
        >{{ label
        }}<span v-if="detail" class="downloadDetail">
          · {{ detail }}</span
        ></span
      >
    </span>
  </div>
</template>
<script setup>
import { computed } from "vue";
const props = defineProps({
  state: { type: String, default: "can_request" },
  busy: Boolean,
  progress: Object,
  queuePosition: Number,
  error: String,
});
defineEmits(["request"]);
const failed = computed(() => ["failed", "error"].includes(props.state));
const label = computed(
  () =>
    ({
      pending: "Requested",
      in_progress: "Downloading",
      failed: "Download failed",
      error: "Request failed",
    })[props.state],
);
const detail = computed(() => {
  if (props.state === "pending" && props.queuePosition > 0)
    return `Queue #${props.queuePosition}`;
  if (props.state === "in_progress" && props.progress?.total_children > 0)
    return `${props.progress.completed ?? 0} of ${props.progress.total_children} tracks`;
  return "";
});
</script>
<style scoped>
.downloadAction {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  min-width: 0;
}
.requestButton {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding: 7px 15px;
  border: 1px solid var(--surface-border-strong);
  border-radius: 999px;
  background: transparent;
  color: var(--text-base);
  font: inherit;
  font-size: var(--text-sm);
  font-weight: 700;
  cursor: pointer;
  transition:
    border-color 150ms,
    transform 150ms;
}
.requestButton:hover:not(:disabled) {
  border-color: var(--text-base);
  transform: scale(1.03);
}
.requestButton:focus-visible {
  outline: 2px solid var(--text-base);
  outline-offset: 4px;
}
.requestButton:disabled {
  opacity: 0.5;
  cursor: wait;
}
svg {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.downloadStatus {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  line-height: 1.5;
}
.active .downloadStatus {
  color: var(--spotify-green);
}
.failed .downloadStatus {
  color: var(--text-negative, #f3727f);
}
.downloadDetail {
  color: var(--text-subdued);
}
@media (prefers-reduced-motion: reduce) {
  .requestButton {
    transition: none;
  }
}
</style>
