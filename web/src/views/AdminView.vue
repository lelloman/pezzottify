<template>
  <div class="adminView">
    <header class="adminHeader">
      <h1 class="adminTitle">Admin Panel</h1>
      <div class="headerActions">
        <div class="connectionStatus" :title="connectionTitle">
          <span class="statusDot" :class="connectionStatusClass"></span>
        </div>
        <router-link to="/" class="closeButton" title="Close Admin Panel">
          <CrossIcon class="closeIcon" />
        </router-link>
      </div>
    </header>
    <div class="adminBody">
      <div v-if="isLoading" class="loadingState">Loading...</div>
      <template v-else>
        <AdminSidebar
          :sections="availableSections"
          :activeSection="activeSection"
        />
        <main class="adminContent">
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
import CrossIcon from "@/components/icons/CrossIcon.vue";
import AdminSidebar from "@/components/admin/AdminSidebar.vue";
import UserManagement from "@/components/admin/UserManagement.vue";
import AnalyticsDashboard from "@/components/admin/AnalyticsDashboard.vue";
import ServerControl from "@/components/admin/ServerControl.vue";
import DownloadManager from "@/components/admin/DownloadManager.vue";
import BatchManager from "@/components/admin/BatchManager.vue";
import BugReports from "@/components/admin/BugReports.vue";
import IngestionManager from "@/components/admin/IngestionManager.vue";
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

// Define available sections based on permissions
const allSections = [
  {
    id: "users",
    label: "Users",
    permission: "ManagePermissions",
    component: UserManagement,
    route: "/admin/users",
  },
  {
    id: "analytics",
    label: "Analytics",
    permission: "ViewAnalytics",
    component: AnalyticsDashboard,
    route: "/admin/analytics",
  },
  {
    id: "server",
    label: "Server",
    permission: "ServerAdmin",
    component: ServerControl,
    route: "/admin/server",
  },
  {
    id: "downloads",
    label: "Downloads",
    permission: "DownloadManagerAdmin",
    component: DownloadManager,
    route: "/admin/downloads",
  },
  {
    id: "batches",
    label: "Batches",
    permission: "EditCatalog",
    component: BatchManager,
    route: "/admin/batches",
  },
  {
    id: "bug-reports",
    label: "Bug Reports",
    permission: "ServerAdmin",
    component: BugReports,
    route: "/admin/bug-reports",
  },
  {
    id: "ingestion",
    label: "Ingestion",
    permission: "EditCatalog",
    component: IngestionManager,
    route: "/admin/ingestion",
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
  height: 100vh;
  background-color: var(--bg-base);
  color: var(--text-base);
}

.adminHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-bottom: 1px solid var(--border-subdued);
  flex-shrink: 0;
  gap: var(--spacing-4);
}

.closeButton {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  color: var(--text-subdued);
  text-decoration: none;
  border-radius: var(--radius-full);
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.closeButton:hover {
  color: var(--text-base);
  background-color: var(--bg-highlight);
}

.closeIcon {
  width: 20px;
  height: 20px;
  stroke: currentColor;
  stroke-width: 2;
}

.adminTitle {
  font-size: var(--text-xl);
  font-weight: var(--font-bold);
  margin: 0;
}

.headerActions {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.connectionStatus {
  display: flex;
  align-items: center;
  justify-content: center;
}

.statusDot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  transition: background-color var(--transition-fast);
}

.status-connected {
  background-color: #22c55e;
  box-shadow: 0 0 6px rgba(34, 197, 94, 0.5);
}

.status-connecting {
  background-color: #f97316;
  box-shadow: 0 0 6px rgba(249, 115, 22, 0.5);
  animation: pulse 1.5s ease-in-out infinite;
}

.status-disconnected {
  background-color: #ef4444;
  box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

.adminBody {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.loadingState {
  display: flex;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-subdued);
}

.adminContent {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-4);
}

/* Responsive: stack sidebar on mobile */
@media (max-width: 768px) {
  .adminBody {
    flex-direction: column;
  }

  .adminContent {
    padding: var(--spacing-3);
  }
}
</style>
