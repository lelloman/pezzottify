<template>
  <div class="serverControl">
    <h2 class="sectionTitle">Server Control</h2>

    <div class="controlCard">
      <div class="controlInfo">
        <h3 class="controlTitle">Restart Server</h3>
        <p class="controlDescription">
          Initiate a server restart. The server will gracefully shut down and
          restart. All connected clients will be temporarily disconnected.
        </p>
      </div>
      <button
        class="rebootButton"
        :disabled="isRebooting"
        @click="showConfirmDialog = true"
      >
        {{ isRebooting ? "Rebooting..." : "Reboot Server" }}
      </button>
    </div>

    <div v-if="rebootError" class="errorMessage">
      {{ rebootError }}
    </div>

    <ConfirmationDialog
      :isOpen="showConfirmDialog"
      :closeCallback="() => (showConfirmDialog = false)"
      :positiveButtonCallback="handleReboot"
      title="Confirm Server Reboot"
      positiveButtonText="Reboot"
      negativeButtonText="Cancel"
    >
      <template #message>
        Are you sure you want to reboot the server? This will disconnect all
        clients temporarily.
      </template>
    </ConfirmationDialog>
  </div>
</template>

<script setup>
import { ref, watch } from "vue";
import { useRemoteStore } from "@/store/remote";
import ConfirmationDialog from "@/components/common/ConfirmationDialog.vue";
import { wsConnectionStatus } from "@/services/websocket";

const remoteStore = useRemoteStore();

const showConfirmDialog = ref(false);
const isRebooting = ref(false);
const rebootError = ref(null);

// Reset rebooting state when connection is restored after a reboot
watch(wsConnectionStatus, (newStatus, oldStatus) => {
  if (
    isRebooting.value &&
    newStatus === "connected" &&
    oldStatus !== "connected"
  ) {
    isRebooting.value = false;
  }
});

const handleReboot = async () => {
  showConfirmDialog.value = false;
  isRebooting.value = true;
  rebootError.value = null;

  const success = await remoteStore.rebootServer();

  if (!success) {
    rebootError.value = "Failed to initiate server reboot. Please try again.";
    isRebooting.value = false;
  }
  // If successful, the server will restart and we'll lose connection
  // The button stays in "Rebooting..." state
};
</script>

<style scoped>
.serverControl {
  max-width: 800px;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-6) 0;
}

.controlCard {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  flex-wrap: wrap;
}

.controlInfo {
  flex: 1;
  min-width: 200px;
}

.controlTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-2) 0;
}

.controlDescription {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  margin: 0;
  line-height: 1.5;
}

.rebootButton {
  padding: var(--spacing-3) var(--spacing-6);
  background-color: #dc2626;
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition:
    background-color var(--transition-fast),
    opacity var(--transition-fast);
  flex-shrink: 0;
}

.rebootButton:hover:not(:disabled) {
  background-color: #b91c1c;
}

.rebootButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.errorMessage {
  margin-top: var(--spacing-4);
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
}
</style>
