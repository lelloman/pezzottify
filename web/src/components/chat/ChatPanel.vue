<script setup>
import { ref, watch, nextTick, onMounted, onUnmounted } from 'vue';
import { useChatStore } from '../../store/chat';
import ChatMessage from './ChatMessage.vue';
import ChatSettings from './ChatSettings.vue';
import LanguagePicker from './LanguagePicker.vue';

const chatStore = useChatStore();

const inputText = ref('');
const messagesContainer = ref(null);
const panelRef = ref(null);
const showSettings = ref(false);

// Panel position and size
const STORAGE_KEY = 'ai_chat_panel_geometry';
const MIN_WIDTH = 320;
const MIN_HEIGHT = 300;
const DEFAULT_WIDTH = 420;
const DEFAULT_HEIGHT = 550;

const panelStyle = ref({
  width: DEFAULT_WIDTH,
  height: DEFAULT_HEIGHT,
  x: null, // null means use default CSS position
  y: null,
});

// Load saved geometry
onMounted(() => {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {
    try {
      const parsed = JSON.parse(saved);
      panelStyle.value = { ...panelStyle.value, ...parsed };
    } catch {
      // ignore
    }
  }
});

// Save geometry on change
function saveGeometry() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(panelStyle.value));
}

// Dragging state
const isDragging = ref(false);
const dragOffset = ref({ x: 0, y: 0 });

function startDrag(e) {
  if (e.target.closest('button') || e.target.closest('input') || e.target.closest('textarea')) {
    return;
  }

  isDragging.value = true;
  const rect = panelRef.value.getBoundingClientRect();

  // Initialize position if not set
  if (panelStyle.value.x === null) {
    panelStyle.value.x = rect.left;
    panelStyle.value.y = rect.top;
  }

  dragOffset.value = {
    x: e.clientX - panelStyle.value.x,
    y: e.clientY - panelStyle.value.y,
  };

  document.addEventListener('mousemove', onDrag);
  document.addEventListener('mouseup', stopDrag);
  e.preventDefault();
}

function onDrag(e) {
  if (!isDragging.value) return;

  const newX = e.clientX - dragOffset.value.x;
  const newY = e.clientY - dragOffset.value.y;

  // Constrain to viewport
  const maxX = window.innerWidth - panelStyle.value.width;
  const maxY = window.innerHeight - panelStyle.value.height;

  panelStyle.value.x = Math.max(0, Math.min(newX, maxX));
  panelStyle.value.y = Math.max(0, Math.min(newY, maxY));
}

function stopDrag() {
  isDragging.value = false;
  document.removeEventListener('mousemove', onDrag);
  document.removeEventListener('mouseup', stopDrag);
  saveGeometry();
}

// Resizing state
const isResizing = ref(false);
const resizeDirection = ref('');

function startResize(e, direction) {
  isResizing.value = true;
  resizeDirection.value = direction;

  const rect = panelRef.value.getBoundingClientRect();

  // Initialize position if not set
  if (panelStyle.value.x === null) {
    panelStyle.value.x = rect.left;
    panelStyle.value.y = rect.top;
  }

  document.addEventListener('mousemove', onResize);
  document.addEventListener('mouseup', stopResize);
  e.preventDefault();
  e.stopPropagation();
}

function onResize(e) {
  if (!isResizing.value) return;

  const dir = resizeDirection.value;
  let { x, y, width, height } = panelStyle.value;

  if (dir.includes('e')) {
    width = Math.max(MIN_WIDTH, e.clientX - x);
  }
  if (dir.includes('w')) {
    const newWidth = Math.max(MIN_WIDTH, (x + width) - e.clientX);
    const newX = x + width - newWidth;
    if (newX >= 0) {
      width = newWidth;
      x = newX;
    }
  }
  if (dir.includes('s')) {
    height = Math.max(MIN_HEIGHT, e.clientY - y);
  }
  if (dir.includes('n')) {
    const newHeight = Math.max(MIN_HEIGHT, (y + height) - e.clientY);
    const newY = y + height - newHeight;
    if (newY >= 0) {
      height = newHeight;
      y = newY;
    }
  }

  // Constrain to viewport
  width = Math.min(width, window.innerWidth - x);
  height = Math.min(height, window.innerHeight - y);

  panelStyle.value = { x, y, width, height };
}

function stopResize() {
  isResizing.value = false;
  resizeDirection.value = '';
  document.removeEventListener('mousemove', onResize);
  document.removeEventListener('mouseup', stopResize);
  saveGeometry();
}

// Reset to default position
function resetPosition() {
  panelStyle.value = {
    width: DEFAULT_WIDTH,
    height: DEFAULT_HEIGHT,
    x: null,
    y: null,
  };
  localStorage.removeItem(STORAGE_KEY);
}

// Cleanup
onUnmounted(() => {
  document.removeEventListener('mousemove', onDrag);
  document.removeEventListener('mouseup', stopDrag);
  document.removeEventListener('mousemove', onResize);
  document.removeEventListener('mouseup', stopResize);
});

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

// Computed style for the panel
function getPanelStyle() {
  const style = {
    width: `${panelStyle.value.width}px`,
    height: `${panelStyle.value.height}px`,
  };

  if (panelStyle.value.x !== null) {
    style.left = `${panelStyle.value.x}px`;
    style.top = `${panelStyle.value.y}px`;
    style.right = 'auto';
    style.bottom = 'auto';
  }

  return style;
}
</script>

<template>
  <Transition name="slide">
    <div
      v-if="chatStore.isOpen"
      ref="panelRef"
      class="chat-panel"
      :class="{ 'chat-panel--dragging': isDragging, 'chat-panel--resizing': isResizing }"
      :style="getPanelStyle()"
    >
      <!-- Resize handles -->
      <div class="resize-handle resize-handle--n" @mousedown="startResize($event, 'n')"></div>
      <div class="resize-handle resize-handle--s" @mousedown="startResize($event, 's')"></div>
      <div class="resize-handle resize-handle--e" @mousedown="startResize($event, 'e')"></div>
      <div class="resize-handle resize-handle--w" @mousedown="startResize($event, 'w')"></div>
      <div class="resize-handle resize-handle--ne" @mousedown="startResize($event, 'ne')"></div>
      <div class="resize-handle resize-handle--nw" @mousedown="startResize($event, 'nw')"></div>
      <div class="resize-handle resize-handle--se" @mousedown="startResize($event, 'se')"></div>
      <div class="resize-handle resize-handle--sw" @mousedown="startResize($event, 'sw')"></div>

      <!-- Settings View -->
      <ChatSettings
        v-if="showSettings"
        @close="showSettings = false"
      />

      <!-- Chat View -->
      <template v-else>
        <!-- Header (draggable) -->
        <div class="chat-panel__header" @mousedown="startDrag">
          <h3>AI Assistant</h3>
          <div class="chat-panel__actions">
            <LanguagePicker />
            <button
              v-if="panelStyle.x !== null"
              class="chat-panel__action"
              title="Reset position"
              @click="resetPosition"
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
                <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                <path d="M3 3v5h5" />
              </svg>
            </button>
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
  width: 420px;
  height: 550px;
  background: var(--bg-elevated);
  border-radius: var(--radius-xl);
  box-shadow: var(--shadow-xl);
  display: flex;
  flex-direction: column;
  z-index: var(--z-modal);
  overflow: hidden;
}

.chat-panel--dragging,
.chat-panel--resizing {
  user-select: none;
}

.chat-panel--dragging {
  cursor: grabbing;
}

/* Resize handles */
.resize-handle {
  position: absolute;
  z-index: 10;
}

.resize-handle--n,
.resize-handle--s {
  left: 10px;
  right: 10px;
  height: 6px;
  cursor: ns-resize;
}

.resize-handle--n { top: 0; }
.resize-handle--s { bottom: 0; }

.resize-handle--e,
.resize-handle--w {
  top: 10px;
  bottom: 10px;
  width: 6px;
  cursor: ew-resize;
}

.resize-handle--e { right: 0; }
.resize-handle--w { left: 0; }

.resize-handle--ne,
.resize-handle--nw,
.resize-handle--se,
.resize-handle--sw {
  width: 12px;
  height: 12px;
}

.resize-handle--ne { top: 0; right: 0; cursor: nesw-resize; }
.resize-handle--nw { top: 0; left: 0; cursor: nwse-resize; }
.resize-handle--se { bottom: 0; right: 0; cursor: nwse-resize; }
.resize-handle--sw { bottom: 0; left: 0; cursor: nesw-resize; }

.chat-panel__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  border-bottom: 1px solid var(--border-default);
  flex-shrink: 0;
  cursor: grab;
}

.chat-panel--dragging .chat-panel__header {
  cursor: grabbing;
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
    /* On mobile, use fixed positioning and ignore saved geometry */
    bottom: calc(var(--player-height-mobile) + var(--mobile-nav-height) + 80px) !important;
    right: 16px !important;
    left: 16px !important;
    top: auto !important;
    width: auto !important;
    height: 60vh !important;
    max-height: calc(100vh - var(--player-height-mobile) - var(--mobile-nav-height) - 100px);
  }

  .chat-panel__header {
    cursor: default;
  }

  .resize-handle {
    display: none;
  }
}
</style>
