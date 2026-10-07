<template>
  <div class="adminView">
    <header class="adminHeader">
      <div class="adminBrand">
        <router-link to="/" class="brandLink">Pezzottify</router-link
        ><span class="brandDivider" aria-hidden="true">/</span>
        <h1 class="adminTitle">Admin</h1>
      </div>
      <div class="headerActions">
        <div
          class="connectionStatus"
          role="status"
          :title="connectionTitle"
          :aria-label="connectionTitle"
        >
          <span
            class="statusDot"
            :class="connectionStatusClass"
            aria-hidden="true"
          ></span
          ><span class="connectionLabel">{{ connectionLabel }}</span>
        </div>
        <router-link
          to="/"
          class="backButton"
          aria-label="Back to player"
          title="Back to player"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m10 5-7 7 7 7M3 12h18" /></svg
          ><span>Back to player</span>
        </router-link>
      </div>
    </header>
    <div class="adminBody">
      <div v-if="isLoading" class="loadingState" role="status">
        Loading admin…
      </div>
      <template v-else>
        <AdminSidebar
          :sections="availableSections"
          :activeSection="activeSection"
        />
        <main
          :key="activeSection"
          class="adminContent"
          aria-label="Admin content"
        >
          <component :is="activeSectionComponent" />
        </main>
      </template>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useUserStore } from "@/store/user";
import AdminSidebar from "@/components/admin/AdminSidebar.vue";
import UserManagement from "@/components/admin/UserManagement.vue";
import AnalyticsDashboard from "@/components/admin/AnalyticsDashboard.vue";
import ServerControl from "@/components/admin/ServerControl.vue";
import DownloadManager from "@/components/admin/DownloadManager.vue";
import BatchManager from "@/components/admin/BatchManager.vue";
import BugReports from "@/components/admin/BugReports.vue";
import IngestionManager from "@/components/admin/IngestionManager.vue";
import PushNotifications from "@/components/admin/PushNotifications.vue";
import { wsConnectionStatus, wsServerVersion } from "@/services/websocket";

const route = useRoute();
const router = useRouter();
const userStore = useUserStore();
const isLoading = ref(true);

// Connection status (same as TopBar)
const connectionStatusClass = computed(() => {
  switch (wsConnectionStatus.value) {
    case "connected":
      return "status-connected";
    case "connecting":
      return "status-connecting";
    default:
      return "status-disconnected";
  }
});

const connectionTitle = computed(() => {
  switch (wsConnectionStatus.value) {
    case "connected":
      return `Connected (Server: v${wsServerVersion.value || "unknown"})`;
    case "connecting":
      return "Connecting...";
    default:
      return "Disconnected";
  }
});

const connectionLabel = computed(
  () =>
    ({ connected: "Connected", connecting: "Connecting…" })[
      wsConnectionStatus.value
    ] || "Disconnected",
);

// Define available sections based on permissions
const allSections = [
  {
    id: "users",
    group: "People",
    label: "Users",
    permission: "ManagePermissions",
    component: UserManagement,
    route: "/admin/users",
  },
  {
    id: "analytics",
    group: "Insights",
    label: "Analytics",
    permission: "ViewAnalytics",
    component: AnalyticsDashboard,
    route: "/admin/analytics",
  },
  {
    id: "server",
    group: "Operations",
    label: "Server",
    permission: "ServerAdmin",
    component: ServerControl,
    route: "/admin/server",
  },
  {
    id: "downloads",
    group: "Operations",
    label: "Downloads",
    permission: "DownloadManagerAdmin",
    component: DownloadManager,
    route: "/admin/downloads",
  },
  {
    id: "batches",
    group: "Catalog",
    label: "Batches",
    permission: "EditCatalog",
    component: BatchManager,
    route: "/admin/batches",
  },
  {
    id: "bug-reports",
    group: "Operations",
    label: "Bug Reports",
    permission: "ServerAdmin",
    component: BugReports,
    route: "/admin/bug-reports",
  },
  {
    id: "ingestion",
    group: "Catalog",
    label: "Ingestion",
    permission: "EditCatalog",
    component: IngestionManager,
    route: "/admin/ingestion",
  },
  {
    id: "push",
    group: "People",
    label: "Push",
    permission: "ServerAdmin",
    component: PushNotifications,
    route: "/admin/push",
  },
];

const availableSections = computed(() => {
  return allSections.filter((section) =>
    userStore.hasPermission(section.permission),
  );
});

// Get active section from route
const activeSection = computed(() => {
  return route.meta.section || null;
});

// Initialize user store and redirect to first available section if needed
onMounted(async () => {
  await userStore.initialize();
  isLoading.value = false;

  // If we're at /admin with no section, redirect to first available
  if (!activeSection.value && availableSections.value.length > 0) {
    router.replace(availableSections.value[0].route);
  }
});

// Watch for permission changes
watch(availableSections, (sections) => {
  if (!activeSection.value && sections.length > 0) {
    router.replace(sections[0].route);
  }
});

const activeSectionComponent = computed(() => {
  const section = allSections.find((s) => s.id === activeSection.value);
  return section?.component || null;
});
</script>

<style scoped>
.adminView {
  display: flex;
  flex-direction: column;
  height: 100dvh;
  overflow: hidden;
  background: #000;
  color: var(--text-base);
}
.adminHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 64px;
  padding: 8px 24px;
  gap: 16px;
  flex-shrink: 0;
}
.adminBrand {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}
.brandLink {
  color: var(--spotify-green);
  font-size: 20px;
  font-weight: 750;
  font-style: italic;
  text-decoration: none;
}
.brandDivider {
  color: #727272;
  font-size: 20px;
}
.adminTitle {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}
.headerActions {
  display: flex;
  align-items: center;
  gap: 24px;
}
.backButton {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 40px;
  padding: 0 16px;
  border: 1px solid #727272;
  border-radius: 999px;
  color: var(--text-base);
  text-decoration: none;
  font-size: 14px;
  font-weight: 700;
  white-space: nowrap;
}
.backButton:hover {
  border-color: #fff;
  background: #1f1f1f;
}
.backButton svg {
  width: 18px;
  height: 18px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
}
a:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
.connectionStatus {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-subdued);
  font-size: 12px;
}
.statusDot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}
.status-connected {
  background: var(--spotify-green);
}
.status-connecting {
  background: #f0bc65;
}
.status-disconnected {
  background: #f3727f;
}
.adminBody {
  display: flex;
  flex: 1;
  min-height: 0;
  min-width: 0;
  gap: 8px;
  padding: 0 8px 8px;
  overflow: hidden;
}
.loadingState {
  display: grid;
  place-items: center;
  flex: 1;
  background: #121212;
  border-radius: 8px;
  color: var(--text-subdued);
}
.adminContent {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: auto;
  overscroll-behavior: contain;
  background: #121212;
  border-radius: 8px;
  padding: 28px 32px;
}
@media (max-width: 767px) {
  .adminHeader {
    min-height: 56px;
    padding: 8px 16px;
    gap: 12px;
  }
  .adminBrand {
    gap: 10px;
  }
  .brandLink {
    font-size: 18px;
  }
  .headerActions {
    gap: 12px;
  }
  .connectionLabel,
  .backButton span {
    display: none;
  }
  .backButton {
    width: 40px;
    padding: 0;
    justify-content: center;
  }
  .adminBody {
    flex-direction: column;
  }
  .adminContent {
    padding: 20px 16px;
  }
}
</style>
