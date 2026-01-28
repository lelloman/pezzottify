<template>
  <div class="review-section">
    <div class="review-header">
      <span class="review-icon">?</span>
      <span class="review-title">Review Required</span>
    </div>

    <div class="review-question">
      {{ review?.question || 'Please select an option' }}
    </div>

    <div class="review-options">
      <button
        v-for="option in review?.options || []"
        :key="option.id"
        class="option-btn"
        :class="{ selected: selectedOption === option.id }"
        :disabled="isSubmitting"
        @click="selectOption(option.id)"
      >
        <span class="option-label">{{ option.label }}</span>
        <span v-if="option.description" class="option-description">
          {{ option.description }}
        </span>
      </button>
    </div>

    <div class="review-actions">
      <button
        class="submit-btn"
        :disabled="!selectedOption || isSubmitting"
        @click="submit"
      >
        {{ isSubmitting ? 'Submitting...' : 'Submit' }}
      </button>
    </div>
  </div>
</template>

<script setup>
import { ref } from "vue";

const props = defineProps({
  review: {
    type: Object,
    default: null,
  },
  jobId: {
    type: String,
    required: true,
  },
});

const emit = defineEmits(["resolve"]);

const selectedOption = ref(null);
const isSubmitting = ref(false);

function selectOption(optionId) {
  selectedOption.value = optionId;
}

async function submit() {
  if (!selectedOption.value || isSubmitting.value) return;

  isSubmitting.value = true;
  try {
    emit("resolve", {
      jobId: props.jobId,
      optionId: selectedOption.value,
    });
  } finally {
    isSubmitting.value = false;
  }
}
</script>

<style scoped>
.review-section {
  margin-top: 16px;
  padding: 16px;
  background: rgba(245, 166, 35, 0.1);
  border: 1px solid #f5a623;
  border-radius: 8px;
}

.review-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.review-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: #f5a623;
  color: var(--bg-base);
  border-radius: 50%;
  font-size: 14px;
  font-weight: bold;
}

.review-title {
  font-weight: 600;
  font-size: 14px;
  color: var(--text-base);
}

.review-question {
  margin-bottom: 16px;
  font-size: 14px;
  color: var(--text-base);
  white-space: pre-line;
}

.review-options {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 16px;
}

.option-btn {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  padding: 12px;
  border: 1px solid var(--border-default);
  border-radius: 6px;
  background: var(--bg-elevated);
  cursor: pointer;
  text-align: left;
}

.option-btn:hover {
  border-color: #4a90d9;
  background: var(--bg-highlight);
}

.option-btn.selected {
  border-color: var(--spotify-green);
  background: rgba(29, 185, 84, 0.1);
}

.option-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.option-label {
  font-size: 14px;
  color: var(--text-base);
  font-weight: 500;
}

.option-description {
  font-size: 12px;
  color: var(--text-subdued);
  margin-top: 4px;
}

.review-actions {
  display: flex;
  justify-content: flex-end;
}

.submit-btn {
  padding: 10px 24px;
  background: var(--spotify-green);
  color: var(--text-negative);
  border: none;
  border-radius: 4px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
}

.submit-btn:hover:not(:disabled) {
  filter: brightness(1.1);
}

.submit-btn:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
