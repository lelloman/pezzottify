<script setup>
import { computed } from "vue";
import { useMaintenanceStore } from "@/store/maintenance";
import { useAuthStore } from "@/store/auth";

const maintenance = useMaintenanceStore();
const authStore = useAuthStore();

const isServerAdmin = computed(
  () => authStore.user?.permissions?.includes("ServerAdmin") === true,
);

const eta = computed(() => {
  if (!maintenance.estimatedEnd) return null;
  const date = new Date(maintenance.estimatedEnd);
  if (Number.isNaN(date.getTime())) return maintenance.estimatedEnd;
  return date.toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
});

function retry() {
  maintenance.fetchStatus();
}
</script>

<template>
  <main class="maintenance" role="main" aria-labelledby="maintenance-title">
    <section class="maintenanceCard">
      <p class="brand">Pezzottify</p>
      <h1 id="maintenance-title">We'll be right back</h1>
      <p class="message">
        {{
          maintenance.message ||
          "Pezzottify is currently undergoing maintenance. Please try again soon."
        }}
      </p>
      <p v-if="eta" class="eta">Expected back: {{ eta }}</p>
      <button type="button" class="retryButton" @click="retry">Retry</button>
      <router-link
        v-if="isServerAdmin"
        class="adminLink"
        to="/admin/maintenance"
      >
        Server administration
      </router-link>
    </section>
  </main>
</template>

<style scoped>
.maintenance {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  width: 100vw;
  padding: 24px;
  background: var(--background);
  color: var(--text-base);
}

.maintenanceCard {
  width: 100%;
  max-width: 520px;
  padding: 48px 40px;
  border-radius: var(--radius-xl);
  background: var(--panel-on-bg);
  box-shadow: var(--shadow-lg);
  text-align: center;
}

.brand {
  margin: 0 0 24px;
  color: var(--spotify-green);
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

h1 {
  margin: 0 0 16px;
  font-size: var(--text-3xl);
  line-height: var(--leading-tight);
}

.message {
  margin: 0;
  color: var(--text-subdued);
  font-size: var(--text-md);
  line-height: var(--leading-relaxed);
}

.eta {
  margin: 20px 0 0;
  color: var(--text-base);
  font-weight: var(--font-semibold);
}

.retryButton {
  margin-top: 32px;
  min-height: 44px;
  padding: 0 32px;
  border: none;
  border-radius: var(--radius-full);
  background: var(--spotify-green);
  color: #000;
  font-size: var(--text-md);
  font-weight: var(--font-bold);
  cursor: pointer;
}

.retryButton:hover {
  background: var(--spotify-green-hover);
}

.retryButton:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}

.adminLink {
  display: block;
  margin-top: 24px;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  text-decoration: underline;
}
</style>
