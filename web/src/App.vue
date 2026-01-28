<script setup>
import { RouterView } from "vue-router";
import { usePlayerStore } from "./store/player";
import { useAuthStore } from "./store/auth";
import { useIngestionStore } from "./store/ingestion";
import { onMounted, onUnmounted, computed, watch } from "vue";
import ChatButton from "./components/chat/ChatButton.vue";
import ChatPanel from "./components/chat/ChatPanel.vue";
import IngestionMonitorModal from "./components/ingestion/IngestionMonitorModal.vue";

const player = usePlayerStore();
const authStore = useAuthStore();
const ingestionStore = useIngestionStore();

// Only show chat when authenticated
const showChat = computed(() => authStore.sessionChecked && authStore.user);

// Initialize ingestion store when authenticated
watch(
  () => authStore.sessionChecked && authStore.user,
  (isAuthenticated) => {
    if (isAuthenticated && !ingestionStore.isInitialized) {
      ingestionStore.initialize();
    } else if (!isAuthenticated && ingestionStore.isInitialized) {
      ingestionStore.cleanup();
    }
  },
  { immediate: true }
);

function handleKeyDown(event) {
  const isEditable =
    event.target.tagName === "INPUT" ||
    event.target.tagName === "TEXTAREA" ||
    event.target.isContentEditable;
  if (!isEditable && event.key === " ") {
    player.playPause();
  }
}
onMounted(() => {
  window.addEventListener("keydown", handleKeyDown);
});
onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
});
</script>

<template>
  <RouterView id="el_routo" />
  <template v-if="showChat">
    <ChatButton />
    <ChatPanel />
  </template>
  <IngestionMonitorModal />
</template>

<style scoped>
#el_routo {
  width: 100vw;
  height: 100vh;
  font-size: 12px;
  text-align: left;
}

nav a.router-link-exact-active {
  color: var(--color-text);
}

nav a.router-link-exact-active:hover {
  background-color: transparent;
}

nav a {
  display: inline-block;
  padding: 0 1rem;
  border-left: 1px solid var(--color-border);
}

nav a:first-of-type {
  border: 0;
}

@media (min-width: 1024px) {
  header {
    display: flex;
    place-items: center;
    padding-right: calc(var(--section-gap) / 2);
  }

  .logo {
    margin: 0 2rem 0 0;
  }

  header .wrapper {
    display: flex;
    place-items: flex-start;
    flex-wrap: wrap;
  }

  nav {
    text-align: left;
    margin-left: -1rem;
    font-size: 1rem;

    padding: 1rem 0;
    margin-top: 1rem;
  }
}
</style>
