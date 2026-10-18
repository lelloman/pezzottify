<script setup>
import { AssistantPanel } from '@lelloman/simple-assistant-vue';
import '@lelloman/simple-assistant-vue/style.css';
import { useChatStore } from '../../store/chat';
import FeedbackForm from '../feedback/FeedbackForm.vue';
const controller = useChatStore();
</script>
<template><AssistantPanel :controller="controller" storage-key="ai_chat_panel_geometry" class="pezzottify-assistant" /><aside v-if="controller.isOpen" class="response-reports" aria-label="Report an assistant response">
  <details><summary>Report a response</summary>
    <button v-for="message in controller.messages.filter(m => m.role === 'assistant')" :key="message.id" @click="controller.reportResponse(message.id)">Report: {{ message.content.slice(0, 80) || 'Tool response' }}</button>
  </details>
</aside><FeedbackForm /></template>
<style>
.response-reports { position: fixed; right: 20px; top: 15px; max-width: 360px; z-index: 1001; background: #181818; padding: 8px; max-height: 30vh; overflow: auto; }
.response-reports button { display: block; }
.pezzottify-assistant {
  --assistant-accent: var(--spotify-green);
  --assistant-accent-hover: var(--spotify-green-hover);
  --assistant-bottom-offset: var(--player-height-desktop);
  --assistant-mobile-bottom-offset: var(--player-height-mobile);
  --assistant-mobile-nav-height: var(--mobile-nav-height);
}
</style>
