<script setup>
import { ref, watch, nextTick } from 'vue';
import { useChatStore } from '../../store/chat';
import ChatMessage from './ChatMessage.vue';
import ChatSettings from './ChatSettings.vue';

const chatStore = useChatStore();

const inputText = ref('');
const messagesContainer = ref(null);
const showSettings = ref(false);

// Auto-scroll to bottom when new messages arrive
watch(
  () => [chatStore.messages.length, chatStore.streamingText],
  async () => {
    await nextTick();
    if (messagesContainer.value) {
      messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight;
    }
  }
);

async function handleSubmit() {
  const text = inputText.value.trim();
  if (!text || chatStore.isLoading) return;

  inputText.value = '';
  await chatStore.sendMessage(text);
}

function handleKeydown(e) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    handleSubmit();
  }
}
</script>

<template>
  <Transition name="slide">
    <div v-if="chatStore.isOpen" class="chat-panel">
      <!-- Settings View -->
      <ChatSettings
        v-if="showSettings"
        @close="showSettings = false"
      />

      <!-- Chat View -->
      <template v-else>
        <!-- Header -->
        <div class="chat-panel__header">
          <h3>AI Assistant</h3>
          <div class="chat-panel__actions">
            <button
              class="chat-panel__action"
              title="Settings"
              @click="showSettings = true"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
            </button>
            <button
              v-if="chatStore.messages.length > 0"
              class="chat-panel__action"
              title="Clear chat"
              @click="chatStore.clearHistory()"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
              >
                <polyline points="3 6 5 6 21 6" />
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Messages -->
        <div ref="messagesContainer" class="chat-panel__messages">
          <!-- Not configured message -->
          <div v-if="!chatStore.isConfigured" class="chat-panel__setup">
            <div class="chat-panel__setup-icon">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="48"
                height="48"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
              >
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
              </svg>
            </div>
            <h4>Configure AI Provider</h4>
            <p>Set up your AI provider to start chatting.</p>
            <button class="chat-panel__setup-btn" @click="showSettings = true">
              Open Settings
            </button>
          </div>

          <!-- Empty state -->
          <div
            v-else-if="chatStore.messages.length === 0"
            class="chat-panel__empty"
          >
            <p>Ask me anything about your music!</p>
            <div class="chat-panel__suggestions">
              <button @click="inputText = 'Search for jazz music'">
                Search for jazz music
              </button>
              <button @click="inputText = 'What is currently playing?'">
                What's playing?
              </button>
              <button @click="inputText = 'Show my liked albums'">
                My liked albums
              </button>
            </div>
          </div>

          <!-- Message list -->
          <template v-else>
            <ChatMessage
              v-for="msg in chatStore.messages"
              :key="msg.id"
              :message="msg"
            />

            <!-- Streaming text -->
            <div v-if="chatStore.streamingText" class="chat-panel__streaming">
              <div class="message__content message__content--assistant">
                {{ chatStore.streamingText }}
                <span class="chat-panel__cursor"></span>
              </div>
            </div>

            <!-- Loading indicator -->
            <div v-else-if="chatStore.isLoading" class="chat-panel__loading">
              <div class="chat-panel__loading-dots">
                <span></span>
                <span></span>
                <span></span>
              </div>
            </div>
          </template>

          <!-- Error message -->
          <div v-if="chatStore.error" class="chat-panel__error">
            {{ chatStore.error }}
          </div>
        </div>

        <!-- Input -->
        <div class="chat-panel__input-container">
          <textarea
            v-model="inputText"
            class="chat-panel__input"
            placeholder="Ask something..."
            :disabled="!chatStore.isConfigured || chatStore.isLoading"
            @keydown="handleKeydown"
            rows="1"
          ></textarea>
          <button
            class="chat-panel__send"
            :disabled="!inputText.trim() || chatStore.isLoading || !chatStore.isConfigured"
            @click="handleSubmit"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
            >
              <line x1="22" y1="2" x2="11" y2="13" />
              <polygon points="22 2 15 22 11 13 2 9 22 2" />
            </svg>
          </button>
        </div>
      </template>
    </div>
  </Transition>
</template>

<style scoped>
.chat-panel {
  position: fixed;
  bottom: calc(var(--player-height-desktop) + 90px);
  right: 20px;
  width: 380px;
  height: 500px;
  max-height: calc(100vh - var(--player-height-desktop) - 120px);
  background: var(--bg-elevated);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xl);
  display: flex;
  flex-direction: column;
  z-index: var(--z-modal);
  overflow: hidden;
}

.chat-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  border-bottom: 1px solid var(--border-default);
  flex-shrink: 0;
}

.chat-panel__header h3 {
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  margin: 0;
}

.chat-panel__actions {
  display: flex;
  gap: var(--spacing-1);
}

.chat-panel__action {
  background: none;
  border: none;
  color: var(--text-subdued);
  cursor: pointer;
  padding: var(--spacing-1);
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
}

.chat-panel__action:hover {
  color: var(--text-base);
  background: var(--bg-highlight);
}

.chat-panel__messages {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-3);
}

.chat-panel__setup,
.chat-panel__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  padding: var(--spacing-4);
}

.chat-panel__setup-icon {
  color: var(--text-subdued);
  margin-bottom: var(--spacing-3);
}

.chat-panel__setup h4,
.chat-panel__empty p {
  color: var(--text-subdued);
  margin-bottom: var(--spacing-2);
}

.chat-panel__setup p {
  font-size: var(--text-sm);
  color: var(--text-subtle);
  margin-bottom: var(--spacing-4);
}

.chat-panel__setup-btn {
  padding: var(--spacing-2) var(--spacing-4);
  background: var(--spotify-green);
  border: none;
  border-radius: var(--radius-md);
  color: var(--text-negative);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.chat-panel__setup-btn:hover {
  background: var(--spotify-green-hover);
}

.chat-panel__suggestions {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  width: 100%;
  max-width: 250px;
}

.chat-panel__suggestions button {
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--bg-highlight);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
  text-align: left;
}

.chat-panel__suggestions button:hover {
  background: var(--bg-press);
  border-color: var(--spotify-green);
}

.chat-panel__streaming .message__content--assistant {
  background: var(--bg-highlight);
  color: var(--text-base);
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-lg);
  border-bottom-left-radius: var(--radius-sm);
  font-size: var(--text-sm);
  white-space: pre-wrap;
}

.chat-panel__cursor {
  display: inline-block;
  width: 2px;
  height: 1em;
  background: var(--text-base);
  margin-left: 2px;
  animation: blink 1s infinite;
}

@keyframes blink {
  0%, 50% { opacity: 1; }
  51%, 100% { opacity: 0; }
}

.chat-panel__loading {
  display: flex;
  justify-content: flex-start;
  padding: var(--spacing-2);
}

.chat-panel__loading-dots {
  display: flex;
  gap: 4px;
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--bg-highlight);
  border-radius: var(--radius-lg);
}

.chat-panel__loading-dots span {
  width: 8px;
  height: 8px;
  background: var(--text-subdued);
  border-radius: 50%;
  animation: bounce 1.4s infinite ease-in-out both;
}

.chat-panel__loading-dots span:nth-child(1) { animation-delay: -0.32s; }
.chat-panel__loading-dots span:nth-child(2) { animation-delay: -0.16s; }

@keyframes bounce {
  0%, 80%, 100% { transform: scale(0); }
  40% { transform: scale(1); }
}

.chat-panel__error {
  margin-top: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-3);
  background: rgba(226, 33, 52, 0.1);
  border-radius: var(--radius-md);
  color: var(--error);
  font-size: var(--text-sm);
}

.chat-panel__input-container {
  display: flex;
  gap: var(--spacing-2);
  padding: var(--spacing-3);
  border-top: 1px solid var(--border-default);
  flex-shrink: 0;
}

.chat-panel__input {
  flex: 1;
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--bg-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  color: var(--text-base);
  font-size: var(--text-sm);
  resize: none;
  min-height: 40px;
  max-height: 100px;
  font-family: inherit;
}

.chat-panel__input:focus {
  outline: none;
  border-color: var(--spotify-green);
}

.chat-panel__input::placeholder {
  color: var(--text-subtle);
}

.chat-panel__input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.chat-panel__send {
  width: 40px;
  height: 40px;
  background: var(--spotify-green);
  border: none;
  border-radius: var(--radius-full);
  color: var(--text-negative);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  transition: all var(--transition-fast);
}

.chat-panel__send:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}

.chat-panel__send:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Slide animation */
.slide-enter-active,
.slide-leave-active {
  transition: all var(--transition-base);
}

.slide-enter-from,
.slide-leave-to {
  opacity: 0;
  transform: translateY(20px) scale(0.95);
}

@media (max-width: 768px) {
  .chat-panel {
    bottom: calc(var(--player-height-mobile) + var(--mobile-nav-height) + 80px);
    right: 16px;
    left: 16px;
    width: auto;
    height: 60vh;
    max-height: calc(100vh - var(--player-height-mobile) - var(--mobile-nav-height) - 100px);
  }
}
</style>
