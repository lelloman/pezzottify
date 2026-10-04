<template>
  <div class="pushNotifications">
    <h2 class="sectionTitle">Push</h2>
    <p class="sectionHint">
      Devices registered for UnifiedPush. A test notification goes straight to
      the device's push distributor and shows up as a system notification.
    </p>

    <div class="messageForm">
      <label class="field">
        <span class="fieldLabel">Title</span>
        <input
          v-model="title"
          class="textInput"
          type="text"
          maxlength="100"
          placeholder="Test notification"
        />
      </label>
      <label class="field">
        <span class="fieldLabel">Message</span>
        <input
          v-model="body"
          class="textInput"
          type="text"
          maxlength="300"
          placeholder="Sent from the Pezzottify admin panel"
        />
      </label>
    </div>

    <div class="actionButtons">
      <input
        v-model="filter"
        class="textInput filterInput"
        type="search"
        placeholder="Filter by user or device"
      />
      <button
        class="refreshButton"
        :disabled="isLoading"
        @click="loadRegistrations"
      >
        {{ isLoading ? "Loading..." : "Refresh" }}
      </button>
    </div>

    <div v-if="isLoading && registrations.length === 0" class="loadingMessage">
      Loading registrations...
    </div>
    <div v-else-if="loadError" class="errorMessage">
      {{ loadError }}
      <button class="retryButton" @click="loadRegistrations">Retry</button>
    </div>
    <div v-else-if="!enabled" class="emptyMessage">
      Push is not configured on this server (no <code>[push]</code> section).
    </div>
    <div v-else-if="registrations.length === 0" class="emptyMessage">
      No device has registered for push notifications yet.
    </div>
    <div v-else-if="visibleRegistrations.length === 0" class="emptyMessage">
      No registration matches "{{ filter }}".
    </div>

    <div v-else class="registrationList">
      <div
        v-for="registration in visibleRegistrations"
        :key="registration.id"
        class="registrationCard"
      >
        <div class="registrationInfo">
          <div class="registrationTitle">
            {{ registration.device_name || "Unknown device" }}
            <span
              v-if="registration.device_type"
              class="clientBadge"
              :class="registration.device_type"
            >
              {{ registration.device_type }}
            </span>
            <span
              v-if="registration.connected"
              class="onlineBadge"
              title="This device has a live WebSocket, so it gets sync events directly"
            >
              online
            </span>
          </div>
          <div class="registrationMeta">
            <span>{{
              registration.user_handle || `user #${registration.user_id}`
            }}</span>
            <span class="separator">•</span>
            <span title="Push distributor host">{{
              registration.endpoint_host
            }}</span>
            <span class="separator">•</span>
            <span>Registered {{ formatDate(registration.created_at) }}</span>
          </div>
          <div class="registrationMeta">
            <span v-if="registration.first_failure_at" class="failing">
              Failing since {{ formatDate(registration.first_failure_at) }}
            </span>
            <span v-else-if="registration.last_success_at">
              Last delivery {{ formatDate(registration.last_success_at) }}
            </span>
            <span v-else>No delivery yet</span>
          </div>
          <div
            v-if="results[registration.id]"
            class="sendResult"
            :class="results[registration.id].kind"
          >
            {{ results[registration.id].text }}
          </div>
        </div>
        <button
          class="sendButton"
          :disabled="sendingId === registration.id"
          @click="sendTest(registration)"
        >
          {{ sendingId === registration.id ? "Sending..." : "Send test" }}
        </button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();

const registrations = ref([]);
const enabled = ref(true);
const isLoading = ref(false);
const loadError = ref(null);
const sendingId = ref(null);
const results = reactive({});
const title = ref("");
const body = ref("");
const filter = ref("");

const visibleRegistrations = computed(() => {
  const needle = filter.value.trim().toLowerCase();
  if (!needle) return registrations.value;
  return registrations.value.filter((registration) =>
    [
      registration.user_handle,
      registration.device_name,
      registration.device_type,
      registration.endpoint_host,
    ].some((value) => value && value.toLowerCase().includes(needle)),
  );
});

const loadRegistrations = async () => {
  isLoading.value = true;
  loadError.value = null;
  const data = await remoteStore.fetchPushRegistrations();
  isLoading.value = false;
  if (!data) {
    loadError.value = "Failed to load push registrations";
    return;
  }
  enabled.value = data.enabled;
  registrations.value = data.registrations || [];
};

const sendTest = async (registration) => {
  sendingId.value = registration.id;
  delete results[registration.id];
  const result = await remoteStore.sendTestPushNotification(registration.id, {
    title: title.value.trim(),
    body: body.value.trim(),
  });
  sendingId.value = null;
  if (result.error) {
    results[registration.id] = { kind: "failure", text: result.error };
  } else if (result.outcome === "delivered") {
    results[registration.id] = {
      kind: "success",
      text: "Accepted by the push distributor",
    };
  } else if (result.outcome === "gone") {
    results[registration.id] = {
      kind: "failure",
      text: "The distributor no longer knows this device; registration removed",
    };
  } else {
    results[registration.id] = {
      kind: "failure",
      text: `Delivery failed${result.detail ? `: ${result.detail}` : ""}`,
    };
  }
  // Delivery bookkeeping (last delivery, removals) changed on the server.
  const data = await remoteStore.fetchPushRegistrations();
  if (data) {
    enabled.value = data.enabled;
    registrations.value = data.registrations || [];
  }
};

const formatDate = (seconds) => {
  if (!seconds) return "";
  return new Date(seconds * 1000).toLocaleString();
};

onMounted(loadRegistrations);
</script>

<style scoped>
.pushNotifications {
  max-width: 900px;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-2) 0;
}

.sectionHint {
  color: var(--text-subdued);
  font-size: var(--text-sm);
  margin: 0 0 var(--spacing-6) 0;
}

.messageForm {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  gap: var(--spacing-3);
  margin-bottom: var(--spacing-4);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.fieldLabel {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.textInput {
  padding: var(--spacing-2) var(--spacing-3);
  background-color: var(--bg-elevated-base);
  color: var(--text-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  min-width: 0;
}

.textInput:focus {
  outline: none;
  border-color: var(--highlight);
}

.actionButtons {
  display: flex;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
}

.filterInput {
  flex: 1;
}

.refreshButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--highlight);
  color: var(--text-base);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.refreshButton:hover:not(:disabled) {
  filter: brightness(1.1);
}

.refreshButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.loadingMessage,
.emptyMessage {
  padding: var(--spacing-4);
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.errorMessage {
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.retryButton {
  padding: var(--spacing-1) var(--spacing-3);
  background-color: #dc2626;
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  cursor: pointer;
}

.registrationList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.registrationCard {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-4);
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
}

.registrationInfo {
  flex: 1;
  min-width: 0;
}

.registrationTitle {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--font-size-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin-bottom: var(--spacing-1);
}

.registrationMeta {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-wrap: wrap;
  margin-top: 2px;
}

.separator {
  color: var(--border-subdued);
}

.failing {
  color: #dc2626;
}

.clientBadge,
.onlineBadge {
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  text-transform: uppercase;
  background-color: var(--bg-highlight);
  color: var(--text-subdued);
}

.clientBadge.android {
  background-color: rgba(61, 220, 132, 0.2);
  color: #3ddc84;
}

.clientBadge.web {
  background-color: rgba(59, 130, 246, 0.2);
  color: #3b82f6;
}

.onlineBadge {
  text-transform: none;
}

.sendResult {
  margin-top: var(--spacing-2);
  font-size: var(--text-xs);
}

.sendResult.success {
  color: #3ddc84;
}

.sendResult.failure {
  color: #dc2626;
}

.sendButton {
  flex-shrink: 0;
  padding: var(--spacing-2) var(--spacing-4);
  background-color: transparent;
  color: var(--text-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.sendButton:hover:not(:disabled) {
  border-color: var(--highlight);
  background-color: var(--bg-highlight);
}

.sendButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

@media (max-width: 768px) {
  .messageForm {
    grid-template-columns: 1fr;
  }

  .registrationCard {
    flex-direction: column;
    align-items: stretch;
  }
}
</style>
