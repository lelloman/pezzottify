<script setup>
import { ref, computed, watch } from 'vue';
import { useChatStore } from '../../store/chat';
import { testConnection, getModels, getProvider } from '../../services/llm';

const emit = defineEmits(['close']);

const chatStore = useChatStore();

// Local state for form
const localConfig = ref({ ...chatStore.config });
const testing = ref(false);
const testResult = ref(null);
const availableModels = ref([]);
const loadingModels = ref(false);

// Update local config when store config changes
watch(() => chatStore.config, (newConfig) => {
  localConfig.value = { ...newConfig };
}, { deep: true });

// Load models when provider changes
watch(() => localConfig.value.provider, async (newProvider) => {
  loadingModels.value = true;
  testResult.value = null;
  try {
    availableModels.value = await getModels(newProvider, localConfig.value);
    // Set default model if not set
    if (!localConfig.value.model && availableModels.value.length > 0) {
      localConfig.value.model = availableModels.value[0].id;
    }
  } catch (e) {
    console.warn('Failed to load models:', e);
    availableModels.value = [];
  }
  loadingModels.value = false;
}, { immediate: true });

// Get provider info based on the LOCAL config (not saved config)
const currentProviderInfo = computed(() => getProvider(localConfig.value.provider));

const requiresApiKey = computed(() =>
  currentProviderInfo.value?.requiresApiKey ?? true
);

const requiresBaseUrl = computed(() =>
  currentProviderInfo.value?.requiresBaseUrl ?? false
);

async function handleTestConnection() {
  testing.value = true;
  testResult.value = null;

  try {
    await testConnection(localConfig.value.provider, localConfig.value);
    testResult.value = { success: true, message: 'Connection successful!' };
  } catch (e) {
    testResult.value = { success: false, message: e.message };
  }

  testing.value = false;
}

function handleSave() {
  chatStore.setConfig(localConfig.value);
  emit('close');
}

function handleCancel() {
  localConfig.value = { ...chatStore.config };
  emit('close');
}
</script>

<template>
  <div class="settings">
    <div class="settings__header">
      <h3>AI Settings</h3>
      <button class="settings__close" @click="handleCancel">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        >
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>

    <div class="settings__body">
      <!-- Provider Selection -->
      <div class="settings__field">
        <label class="settings__label">Provider</label>
        <select v-model="localConfig.provider" class="settings__select">
          <option
            v-for="p in chatStore.availableProviders"
            :key="p.id"
            :value="p.id"
          >
            {{ p.name }}
          </option>
        </select>
      </div>

      <!-- API Key (if required) -->
      <div v-if="requiresApiKey" class="settings__field">
        <label class="settings__label">API Key</label>
        <input
          v-model="localConfig.apiKey"
          type="password"
          class="settings__input"
          placeholder="Enter your API key"
        />
        <p class="settings__hint">
          Your API key is stored locally in your browser.
        </p>
      </div>

      <!-- Base URL (for Ollama) -->
      <div v-if="requiresBaseUrl" class="settings__field">
        <label class="settings__label">Base URL</label>
        <input
          v-model="localConfig.baseUrl"
          type="text"
          class="settings__input"
          placeholder="http://localhost:11434"
        />
      </div>

      <!-- Model Selection -->
      <div class="settings__field">
        <label class="settings__label">Model</label>
        <select
          v-model="localConfig.model"
          class="settings__select"
          :disabled="loadingModels"
        >
          <option v-if="loadingModels" value="">Loading models...</option>
          <option
            v-for="m in availableModels"
            :key="m.id"
            :value="m.id"
          >
            {{ m.name }}
          </option>
        </select>
      </div>

      <!-- Test Connection -->
      <div class="settings__field">
        <button
          class="settings__test-btn"
          :disabled="testing || (!localConfig.apiKey && requiresApiKey)"
          @click="handleTestConnection"
        >
          {{ testing ? 'Testing...' : 'Test Connection' }}
        </button>
        <div
          v-if="testResult"
          class="settings__test-result"
          :class="{ 'is-success': testResult.success, 'is-error': !testResult.success }"
        >
          {{ testResult.message }}
        </div>
      </div>
    </div>

    <div class="settings__footer">
      <button class="settings__btn settings__btn--secondary" @click="handleCancel">
        Cancel
      </button>
      <button class="settings__btn settings__btn--primary" @click="handleSave">
        Save
      </button>
    </div>
  </div>
</template>

<style scoped>
.settings {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.settings__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-3) var(--spacing-4);
  border-bottom: 1px solid var(--border-default);
}

.settings__header h3 {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  margin: 0;
}

.settings__close {
  background: none;
  border: none;
  color: var(--text-subdued);
  cursor: pointer;
  padding: var(--spacing-1);
  border-radius: var(--radius-sm);
}

.settings__close:hover {
  color: var(--text-base);
  background: var(--bg-highlight);
}

.settings__body {
  flex: 1;
  padding: var(--spacing-4);
  overflow-y: auto;
}

.settings__field {
  margin-bottom: var(--spacing-4);
}

.settings__label {
  display: block;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-base);
  margin-bottom: var(--spacing-1);
}

.settings__input,
.settings__select {
  width: 100%;
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--bg-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-sm);
}

.settings__input:focus,
.settings__select:focus {
  outline: none;
  border-color: var(--spotify-green);
}

.settings__input::placeholder {
  color: var(--text-subtle);
}

.settings__hint {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  margin-top: var(--spacing-1);
}

.settings__test-btn {
  padding: var(--spacing-2) var(--spacing-3);
  background: var(--bg-highlight);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.settings__test-btn:hover:not(:disabled) {
  background: var(--bg-press);
}

.settings__test-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.settings__test-result {
  margin-top: var(--spacing-2);
  padding: var(--spacing-2);
  border-radius: var(--radius-sm);
  font-size: var(--text-sm);
}

.settings__test-result.is-success {
  background: rgba(29, 185, 84, 0.1);
  color: var(--success);
}

.settings__test-result.is-error {
  background: rgba(226, 33, 52, 0.1);
  color: var(--error);
}

.settings__footer {
  display: flex;
  gap: var(--spacing-2);
  padding: var(--spacing-3) var(--spacing-4);
  border-top: 1px solid var(--border-default);
}

.settings__btn {
  flex: 1;
  padding: var(--spacing-2) var(--spacing-3);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.settings__btn--secondary {
  background: transparent;
  border: 1px solid var(--border-default);
  color: var(--text-base);
}

.settings__btn--secondary:hover {
  background: var(--bg-highlight);
}

.settings__btn--primary {
  background: var(--spotify-green);
  border: none;
  color: var(--text-negative);
}

.settings__btn--primary:hover {
  background: var(--spotify-green-hover);
}
</style>
