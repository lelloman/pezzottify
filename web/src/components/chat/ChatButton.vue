<script setup>
import { useChatStore } from '../../store/chat';

const chatStore = useChatStore();
</script>

<template>
  <button
    class="chat-button"
    :class="{ 'is-open': chatStore.isOpen }"
    @click="chatStore.toggle()"
    title="AI Assistant"
  >
    <svg
      v-if="!chatStore.isOpen"
      xmlns="http://www.w3.org/2000/svg"
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
      <circle cx="9" cy="10" r="1" fill="currentColor" />
      <circle cx="12" cy="10" r="1" fill="currentColor" />
      <circle cx="15" cy="10" r="1" fill="currentColor" />
    </svg>
    <svg
      v-else
      xmlns="http://www.w3.org/2000/svg"
      width="24"
      height="24"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  </button>
</template>

<style scoped>
.chat-button {
  position: fixed;
  bottom: calc(var(--player-height-desktop) + 20px);
  right: 20px;
  width: 56px;
  height: 56px;
  border-radius: var(--radius-full);
  background: var(--spotify-green);
  color: var(--text-negative);
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: var(--shadow-lg);
  transition: all var(--transition-fast);
  z-index: var(--z-fixed);
}

.chat-button:hover {
  background: var(--spotify-green-hover);
  transform: scale(1.05);
}

.chat-button:active {
  background: var(--spotify-green-active);
  transform: scale(0.95);
}

.chat-button.is-open {
  background: var(--bg-highlight);
}

.chat-button.is-open:hover {
  background: var(--bg-press);
}

@media (max-width: 768px) {
  .chat-button {
    bottom: calc(var(--player-height-mobile) + var(--mobile-nav-height) + 16px);
    right: 16px;
    width: 48px;
    height: 48px;
  }
}
</style>
