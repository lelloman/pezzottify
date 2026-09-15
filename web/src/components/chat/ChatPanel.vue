<script setup>
import { AssistantPanel } from '@lelloman/simple-assistant-vue';
import '@lelloman/simple-assistant-vue/style.css';
import { useChatStore } from '../../store/chat';
const controller = useChatStore();
</script>
<template><AssistantPanel :controller="controller" storage-key="ai_chat_panel_geometry" class="pezzottify-assistant" /></template>
<style>
.pezzottify-assistant {
  --assistant-accent: var(--spotify-green);
  --assistant-accent-hover: var(--spotify-green-hover);
  --assistant-bottom-offset: var(--player-height-desktop);
  --assistant-mobile-bottom-offset: var(--player-height-mobile);
  --assistant-mobile-nav-height: var(--mobile-nav-height);
}
</style>
