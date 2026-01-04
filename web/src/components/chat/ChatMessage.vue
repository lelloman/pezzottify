<script setup>
import { ref, computed } from 'vue';
import { useChatStore } from '../../store/chat';
import { getToolDescription, getToolResultDescription } from '../../services/toolDescriptions';

const props = defineProps({
  message: {
    type: Object,
    required: true,
  },
});

const chatStore = useChatStore();
const showToolDetails = ref(false);

const isUser = computed(() => props.message.role === 'user');
const isAssistant = computed(() => props.message.role === 'assistant');
const isTool = computed(() => props.message.role === 'tool');

const hasToolCalls = computed(() =>
  props.message.toolCalls && props.message.toolCalls.length > 0
);

// Parse tool result content
const toolResultParsed = computed(() => {
  if (!isTool.value) return null;
  try {
    return JSON.parse(props.message.content);
  } catch {
    return props.message.content;
  }
});

// Get friendly description for a tool call
function getFriendlyToolDescription(tc) {
  return getToolDescription(tc.name, tc.input);
}

// Get friendly result description
const friendlyToolResult = computed(() => {
  if (!isTool.value) return null;
  return getToolResultDescription(props.message.toolName, props.message.content);
});
</script>

<template>
  <div
    class="message"
    :class="{
      'message--user': isUser,
      'message--assistant': isAssistant,
      'message--tool': isTool,
    }"
  >
    <!-- User message -->
    <div v-if="isUser" class="message__content message__content--user">
      {{ message.content }}
    </div>

    <!-- Assistant message -->
    <div v-else-if="isAssistant" class="message__content message__content--assistant">
      <div v-if="message.content" class="message__text">
        {{ message.content }}
      </div>

      <!-- Tool calls indicator -->
      <div v-if="hasToolCalls" class="message__tools">
        <!-- Debug mode: expandable technical details -->
        <template v-if="chatStore.debugMode">
          <button
            class="message__tools-toggle"
            @click="showToolDetails = !showToolDetails"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
            </svg>
            {{ message.toolCalls.length }} tool{{ message.toolCalls.length > 1 ? 's' : '' }} used
            <svg
              class="message__tools-chevron"
              :class="{ 'is-open': showToolDetails }"
              xmlns="http://www.w3.org/2000/svg"
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <polyline points="6 9 12 15 18 9" />
            </svg>
          </button>

          <div v-if="showToolDetails" class="message__tools-list">
            <div
              v-for="tc in message.toolCalls"
              :key="tc.id"
              class="message__tool-call"
            >
              <div class="message__tool-name">{{ tc.name }}</div>
              <pre class="message__tool-input">{{ JSON.stringify(tc.input, null, 2) }}</pre>
            </div>
          </div>
        </template>

        <!-- Regular mode: friendly descriptions -->
        <template v-else>
          <div class="message__tools-friendly">
            <span
              v-for="tc in message.toolCalls"
              :key="tc.id"
              class="message__tool-friendly"
            >
              {{ getFriendlyToolDescription(tc) }}
            </span>
          </div>
        </template>
      </div>
    </div>

    <!-- Tool result -->
    <div v-else-if="isTool" class="message__content message__content--tool">
      <!-- Debug mode: technical details -->
      <template v-if="chatStore.debugMode">
        <div class="message__tool-result">
          <span class="message__tool-label">{{ message.toolName }}</span>
          <span v-if="toolResultParsed?.success" class="message__tool-success">success</span>
          <span v-else-if="toolResultParsed?.error" class="message__tool-error">error</span>
        </div>
      </template>

      <!-- Regular mode: friendly result -->
      <template v-else>
        <div class="message__tool-result message__tool-result--friendly">
          <span :class="toolResultParsed?.error ? 'message__tool-error' : 'message__tool-success'">
            {{ friendlyToolResult }}
          </span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.message {
  display: flex;
  margin-bottom: var(--spacing-2);
}

.message--user {
  justify-content: flex-end;
}

.message--assistant {
  justify-content: flex-start;
}

.message--tool {
  justify-content: flex-start;
}

.message__content {
  max-width: 85%;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-lg);
  font-size: var(--text-sm);
  line-height: var(--leading-normal);
}

.message__content--user {
  background: var(--spotify-green);
  color: var(--text-negative);
  border-bottom-right-radius: var(--radius-sm);
}

.message__content--assistant {
  background: var(--bg-highlight);
  color: var(--text-base);
  border-bottom-left-radius: var(--radius-sm);
}

.message__content--tool {
  background: var(--bg-elevated);
  color: var(--text-subdued);
  font-size: var(--text-xs);
  padding: var(--spacing-1) var(--spacing-2);
}

.message__text {
  white-space: pre-wrap;
  word-break: break-word;
}

.message__tools {
  margin-top: var(--spacing-2);
  border-top: 1px solid var(--border-subtle);
  padding-top: var(--spacing-2);
}

.message__tools-toggle {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  background: none;
  border: none;
  color: var(--text-subdued);
  font-size: var(--text-xs);
  cursor: pointer;
  padding: 0;
}

.message__tools-toggle:hover {
  color: var(--text-base);
}

.message__tools-chevron {
  transition: transform var(--transition-fast);
}

.message__tools-chevron.is-open {
  transform: rotate(180deg);
}

.message__tools-list {
  margin-top: var(--spacing-2);
}

.message__tool-call {
  background: var(--bg-base);
  border-radius: var(--radius-sm);
  padding: var(--spacing-2);
  margin-bottom: var(--spacing-1);
}

.message__tool-name {
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  color: var(--spotify-green);
  margin-bottom: var(--spacing-1);
}

.message__tool-input {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  margin: 0;
  overflow-x: auto;
  font-family: monospace;
}

.message__tool-result {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.message__tool-label {
  font-family: monospace;
}

.message__tool-success {
  color: var(--success);
}

.message__tool-error {
  color: var(--error);
}

/* Friendly mode styles */
.message__tools-friendly {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-1);
}

.message__tool-friendly {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  font-style: italic;
}

.message__tool-result--friendly {
  font-style: italic;
}
</style>
