<template>
  <div class="maintenanceControl">
    <header class="pageHeader">
      <h2 class="pageTitle">Maintenance mode</h2>
      <p>
        Temporarily hide the player for everyone while the server is being
        worked on. Authentication, the status endpoint and this admin panel stay
        available so you can turn it off again.
      </p>
    </header>

    <div v-if="isLoading" class="loadingMessage" role="status">
      Loading maintenance state…
    </div>
    <div v-else-if="loadError" class="errorMessage" role="alert">
      {{ loadError }}
      <button class="secondaryButton" type="button" @click="load">
        Retry
      </button>
    </div>
    <form v-else class="controlCard" @submit.prevent="save">
      <div class="controlInfo">
        <h3 class="controlTitle">
          Current state:
          <span :class="form.enabled ? 'stateEnabled' : 'stateDisabled'">{{
            form.enabled ? "enabled" : "disabled"
          }}</span>
        </h3>
        <p class="controlDescription">
          While enabled, player API requests return 503. Clients show only the
          maintenance message and the expected return time.
        </p>
      </div>

      <div class="formFields">
        <label class="toggleField">
          <input v-model="form.enabled" type="checkbox" />
          <span>Enable maintenance mode</span>
        </label>

        <label class="field">
          <span class="fieldLabel">Message</span>
          <textarea
            v-model="form.message"
            class="textInput"
            rows="3"
            maxlength="2000"
            placeholder="Pezzottify is currently undergoing maintenance."
          ></textarea>
        </label>

        <label class="field">
          <span class="fieldLabel">Expected end (optional)</span>
          <input
            v-model="form.estimatedEnd"
            class="textInput"
            type="datetime-local"
          />
        </label>

        <div class="actions">
          <button class="primaryButton" type="submit" :disabled="isSaving">
            {{ isSaving ? "Saving…" : "Save" }}
          </button>
          <button
            class="secondaryButton"
            type="button"
            :disabled="isSaving"
            @click="load"
          >
            Reset
          </button>
          <span v-if="saved" class="saveSuccess" role="status">Saved!</span>
          <span v-if="saveError" class="saveError" role="alert">{{
            saveError
          }}</span>
        </div>
      </div>
    </form>
  </div>
</template>

<script setup>
import { onMounted, reactive, ref } from "vue";
import { useRemoteStore } from "@/store/remote";
import { useMaintenanceStore } from "@/store/maintenance";

const remote = useRemoteStore();
const maintenance = useMaintenanceStore();

const isLoading = ref(true);
const isSaving = ref(false);
const loadError = ref(null);
const saveError = ref(null);
const saved = ref(false);

const form = reactive({
  enabled: false,
  message: "",
  estimatedEnd: "",
});

function toLocalInput(value) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const local = new Date(date.getTime() - date.getTimezoneOffset() * 60000);
  return local.toISOString().slice(0, 16);
}

function toIso(value) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date.toISOString();
}

function apply(data) {
  form.enabled = data.enabled === true;
  form.message = data.message || "";
  form.estimatedEnd = toLocalInput(data.estimated_end);
}

async function load() {
  isLoading.value = true;
  loadError.value = null;
  saveError.value = null;
  saved.value = false;
  const data = await remote.fetchMaintenance();
  if (data) {
    apply(data);
  } else {
    loadError.value = "Failed to load maintenance state.";
  }
  isLoading.value = false;
}

async function save() {
  isSaving.value = true;
  saveError.value = null;
  saved.value = false;
  const result = await remote.updateMaintenance({
    enabled: form.enabled,
    message: form.message.trim() || null,
    estimated_end: toIso(form.estimatedEnd),
  });
  if (result.success) {
    apply(result.data);
    maintenance.apply(result.data);
    saved.value = true;
  } else {
    saveError.value = result.error;
  }
  isSaving.value = false;
}

onMounted(load);
</script>

<style scoped>
.maintenanceControl {
  width: 100%;
  min-width: 0;
  color: var(--text-base);
  color-scheme: dark;
}

.pageHeader {
  margin-bottom: 28px;
}

.pageTitle {
  margin: 0 0 10px;
  font-size: 32px;
  font-weight: 700;
  line-height: 1.2;
  letter-spacing: -0.03em;
}

.pageHeader p {
  margin: 0;
  max-width: 720px;
  font-size: 14px;
  line-height: 1.5;
  color: var(--text-subdued);
}

.loadingMessage,
.errorMessage {
  padding: 24px;
  border-radius: 8px;
  background: #181818;
  font-size: 14px;
}

.errorMessage {
  color: var(--error);
}

.controlCard {
  display: flex;
  flex-wrap: wrap;
  gap: 24px;
  padding: 24px;
  border-radius: 8px;
  background: #181818;
}

.controlInfo {
  flex: 1 1 260px;
  min-width: 0;
}

.controlTitle {
  margin: 0 0 10px;
  font-size: 18px;
  font-weight: 700;
}

.controlDescription {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-subdued);
}

.stateEnabled {
  color: var(--warning);
}

.stateDisabled {
  color: var(--spotify-green);
}

.formFields {
  display: flex;
  flex: 1 1 360px;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.fieldLabel {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-subdued);
}

.toggleField {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 14px;
  font-weight: 600;
}

.toggleField input {
  width: 18px;
  height: 18px;
  accent-color: var(--spotify-green);
}

.textInput {
  width: 100%;
  box-sizing: border-box;
  padding: 10px 12px;
  border: 1px solid var(--border-strong);
  border-radius: 6px;
  background: #121212;
  color: var(--text-base);
  font: inherit;
  font-size: 14px;
}

.textInput:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 2px;
}

.actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}

button {
  min-height: 40px;
  padding: 0 24px;
  border: none;
  border-radius: 999px;
  font: inherit;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}

button:disabled {
  opacity: 0.45;
  cursor: default;
}

button:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}

.primaryButton {
  background: var(--spotify-green);
  color: #000;
}

.primaryButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}

.secondaryButton {
  border: 1px solid var(--border-strong);
  background: transparent;
  color: var(--text-base);
}

.saveSuccess {
  color: var(--spotify-green);
  font-size: 13px;
  font-weight: 600;
}

.saveError {
  color: var(--error);
  font-size: 13px;
}
</style>
