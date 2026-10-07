<template>
  <div class="pushNotifications">
    <header class="pageHeader">
      <div>
        <h2 class="sectionTitle">Push notifications</h2>
        <p>
          Check registered devices and send a test notification through
          UnifiedPush.
        </p>
      </div>
      <button
        class="refreshButton"
        :disabled="isLoading"
        @click="loadRegistrations"
      >
        {{ isLoading ? "Loading…" : "Refresh" }}
      </button>
    </header>

    <section class="messageSection" aria-labelledby="message-heading">
      <h3 id="message-heading">Test message</h3>
      <p class="sectionHint">
        Choose a device below to send it a system notification. Leave these
        fields empty to use the default message.
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
          <textarea
            v-model="body"
            class="textInput"
            rows="3"
            maxlength="300"
            placeholder="Sent from the Pezzottify admin panel"
          ></textarea>
        </label>
      </div>
    </section>
    <div class="listHeader">
      <h3>
        Registered devices
        <span v-if="!isLoading && !loadError && enabled" class="deviceCount">{{
          registrations.length
        }}</span>
      </h3>
      <input
        v-model="filter"
        class="textInput filterInput"
        type="search"
        aria-label="Filter registered devices"
        placeholder="Filter by user or device"
      />
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
            role="status"
            :class="results[registration.id].kind"
          >
            {{ results[registration.id].text }}
          </div>
        </div>
        <button
          class="sendButton"
          :disabled="sendingId !== null || !enabled"
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
  if (sendingId.value !== null) return;
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
  width: 100%;
  min-width: 0;
  color: var(--text-base);
}
.pageHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.04em;
}
.pageHeader p,
.sectionHint {
  margin: 8px 0 0;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.5;
}
h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.messageSection {
  padding: 24px;
  margin-bottom: 28px;
  background: #181818;
  border-radius: 8px;
}
.messageForm {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
  align-items: start;
  gap: 24px;
  margin-top: 24px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.fieldLabel {
  font-size: 13px;
  font-weight: 600;
}
.textInput {
  box-sizing: border-box;
  min-width: 0;
  min-height: 44px;
  padding: 12px;
  border: 1px solid #727272;
  border-radius: 4px;
  color: #fff;
  background: #242424;
  font: inherit;
  font-size: 14px;
}
.textInput::placeholder {
  color: #b3b3b3;
  opacity: 1;
}
.textInput:hover {
  border-color: #b3b3b3;
}
.textInput:focus {
  outline: 2px solid #fff;
  outline-offset: -2px;
}
textarea.textInput {
  resize: vertical;
  line-height: 1.5;
}
.listHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 16px;
  padding-bottom: 20px;
  margin-bottom: 20px;
  border-bottom: 1px solid #282828;
}
.deviceCount {
  margin-left: 8px;
  font-size: 14px;
  color: var(--text-subdued);
  font-weight: 400;
}
.filterInput {
  width: min(100%, 340px);
}
button {
  font: inherit;
  cursor: pointer;
}
button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
button:focus-visible {
  outline: 2px solid white;
  outline-offset: 3px;
}
.refreshButton,
.retryButton,
.sendButton {
  min-height: 40px;
  padding: 9px 20px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: white;
  font-size: 13px;
  font-weight: 700;
  flex-shrink: 0;
}
.refreshButton:hover:not(:disabled),
.retryButton:hover,
.sendButton:hover:not(:disabled) {
  border-color: #fff;
  background: #242424;
}
.loadingMessage,
.emptyMessage {
  padding: 40px 20px;
  border-radius: 8px;
  background: #181818;
  color: var(--text-subdued);
  font-size: 14px;
  text-align: center;
}
.errorMessage {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 16px;
  padding: 16px;
  border-radius: 8px;
  color: #f3727f;
  background: #281a1d;
  font-size: 14px;
}
.registrationList {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.registrationCard {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  padding: 20px;
  border-radius: 8px;
  background: #181818;
}
.registrationInfo {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.registrationTitle {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 10px;
}
.registrationMeta {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: 6px;
  color: var(--text-subdued);
  font-size: 12px;
  line-height: 1.5;
}
.separator {
  color: #727272;
}
.clientBadge {
  padding: 3px 8px;
  border-radius: 4px;
  background: #2a2a2a;
  color: #b3b3b3;
  text-transform: capitalize;
  font-size: 12px;
  font-weight: 400;
}
.onlineBadge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: #b3b3b3;
  font-size: 12px;
  font-weight: 400;
}
.onlineBadge::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #1ed760;
}
.failing,
.sendResult.failure {
  color: #f3727f;
}
.sendResult {
  margin-top: 12px;
  font-size: 13px;
  line-height: 1.5;
}
.sendResult.success {
  color: #1ed760;
}
@media (max-width: 700px) {
  .pageHeader {
    flex-direction: column;
    align-items: flex-start;
  }
  .messageForm {
    grid-template-columns: minmax(0, 1fr);
    gap: 20px;
  }
  .messageSection {
    padding: 20px;
  }
  .registrationCard {
    flex-direction: column;
    align-items: flex-start;
    gap: 16px;
  }
  .filterInput {
    width: 100%;
  }
}
@media (max-width: 400px) {
  .messageSection,
  .registrationCard {
    padding: 16px;
  }
}
</style>
