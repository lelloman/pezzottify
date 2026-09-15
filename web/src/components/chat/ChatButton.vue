<script setup>
import { AssistantButton } from '@lelloman/simple-assistant-vue';
import '@lelloman/simple-assistant-vue/style.css';
import { useChatStore } from '../../store/chat';
const controller = useChatStore();
</script>
<template><AssistantButton :controller="controller" class="pezzottify-assistant" /></template>
