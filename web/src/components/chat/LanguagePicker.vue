<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useChatStore } from '../../store/chat';

const chatStore = useChatStore();

const isOpen = ref(false);
const searchQuery = ref('');
const pickerRef = ref(null);

const filteredLanguages = computed(() => {
  const query = searchQuery.value.toLowerCase();
  if (!query) return chatStore.availableLanguages;

  return chatStore.availableLanguages.filter(lang =>
    lang.name.toLowerCase().includes(query) ||
    lang.nativeName.toLowerCase().includes(query) ||
    lang.code.toLowerCase().includes(query)
  );
});

function toggleDropdown() {
  isOpen.value = !isOpen.value;
  if (isOpen.value) {
    searchQuery.value = '';
  }
}

function selectLanguage(code) {
  chatStore.setLanguage(code);
  isOpen.value = false;
}

function resetLanguage() {
  chatStore.resetLanguage();
  isOpen.value = false;
}

function handleClickOutside(e) {
  if (isOpen.value && pickerRef.value && !pickerRef.value.contains(e.target)) {
    isOpen.value = false;
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside);
});
</script>

<template>
  <div ref="pickerRef" class="language-picker">
    <button
      class="language-picker__trigger"
      :class="{ 'language-picker__trigger--detecting': chatStore.isDetectingLanguage }"
      :title="chatStore.currentLanguage ? `Language: ${chatStore.currentLanguage.name}` : 'Language not set (will auto-detect)'"
      @click="toggleDropdown"
    >
      <span v-if="chatStore.isDetectingLanguage" class="language-picker__detecting">
        <span class="language-picker__spinner"></span>
      </span>
      <span v-else-if="chatStore.currentLanguage" class="language-picker__flag">
        {{ chatStore.currentLanguage.flag }}
      </span>
      <span v-else class="language-picker__unknown">?</span>
    </button>

    <Transition name="dropdown">
      <div v-if="isOpen" class="language-picker__dropdown">
        <div class="language-picker__search">
          <input
            v-model="searchQuery"
            type="text"
            placeholder="Search languages..."
            @click.stop
          />
        </div>

        <div class="language-picker__list">
          <!-- Reset option -->
          <button
            v-if="chatStore.currentLanguage"
            class="language-picker__item language-picker__item--reset"
            @click="resetLanguage"
          >
            <span class="language-picker__item-flag">?</span>
            <span class="language-picker__item-name">Auto-detect</span>
            <span class="language-picker__item-hint">Reset</span>
          </button>

          <!-- Language options -->
          <button
            v-for="lang in filteredLanguages"
            :key="lang.code"
            class="language-picker__item"
            :class="{ 'language-picker__item--selected': chatStore.config.language === lang.code }"
            @click="selectLanguage(lang.code)"
          >
            <span class="language-picker__item-flag">{{ lang.flag }}</span>
            <span class="language-picker__item-name">{{ lang.name }}</span>
            <span class="language-picker__item-native">{{ lang.nativeName }}</span>
          </button>

          <div v-if="filteredLanguages.length === 0" class="language-picker__empty">
            No languages match "{{ searchQuery }}"
          </div>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.language-picker {
  position: relative;
}

.language-picker__trigger {
  background: none;
  border: none;
  color: var(--text-subdued);
  cursor: pointer;
  padding: var(--spacing-1);
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 28px;
  height: 28px;
  font-size: 16px;
}

.language-picker__trigger:hover {
  color: var(--text-base);
  background: var(--bg-highlight);
}

.language-picker__trigger--detecting {
  cursor: wait;
}

.language-picker__flag {
  font-size: 18px;
  line-height: 1;
}

.language-picker__unknown {
  font-size: var(--text-md);
  font-weight: var(--font-bold);
  color: var(--text-subtle);
}

.language-picker__detecting {
  display: flex;
  align-items: center;
  justify-content: center;
}

.language-picker__spinner {
  width: 14px;
  height: 14px;
  border: 2px solid var(--text-subtle);
  border-top-color: var(--spotify-green);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.language-picker__dropdown {
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: var(--spacing-1);
  width: 220px;
  max-height: 320px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-lg);
  z-index: 100;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.language-picker__search {
  padding: var(--spacing-2);
  border-bottom: 1px solid var(--border-default);
}

.language-picker__search input {
  width: 100%;
  padding: var(--spacing-1) var(--spacing-2);
  background: var(--bg-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-sm);
  color: var(--text-base);
  font-size: var(--text-sm);
}

.language-picker__search input:focus {
  outline: none;
  border-color: var(--spotify-green);
}

.language-picker__search input::placeholder {
  color: var(--text-subtle);
}

.language-picker__list {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-1);
}

.language-picker__item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  width: 100%;
  padding: var(--spacing-1) var(--spacing-2);
  background: none;
  border: none;
  border-radius: var(--radius-sm);
  color: var(--text-base);
  cursor: pointer;
  text-align: left;
  font-size: var(--text-sm);
}

.language-picker__item:hover {
  background: var(--bg-highlight);
}

.language-picker__item--selected {
  background: var(--bg-press);
}

.language-picker__item--reset {
  border-bottom: 1px solid var(--border-default);
  margin-bottom: var(--spacing-1);
  padding-bottom: var(--spacing-2);
  border-radius: 0;
}

.language-picker__item--reset:hover {
  background: var(--bg-highlight);
}

.language-picker__item-flag {
  font-size: 16px;
  width: 20px;
  text-align: center;
}

.language-picker__item-name {
  flex: 1;
  font-weight: var(--font-medium);
}

.language-picker__item-native {
  color: var(--text-subdued);
  font-size: var(--text-xs);
}

.language-picker__item-hint {
  color: var(--text-subtle);
  font-size: var(--text-xs);
}

.language-picker__empty {
  padding: var(--spacing-3);
  text-align: center;
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

/* Dropdown animation */
.dropdown-enter-active,
.dropdown-leave-active {
  transition: all 0.15s ease;
}

.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
