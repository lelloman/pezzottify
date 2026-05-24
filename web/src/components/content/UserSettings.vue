<template>
  <div class="settingsPage">
    <h1 class="pageTitle">Settings</h1>

    <div class="settingsSection">
      <h2 class="sectionTitle">Search</h2>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Organic Search</span>
          <span class="settingDescription">
            Use classic flat search results. When disabled, uses smart search
            with intelligent result grouping and enrichment.
          </span>
        </div>
        <label class="toggle">
          <input type="checkbox" v-model="useOrganicSearch" />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Hide Unavailable Content</span>
          <span class="settingDescription">
            Hide tracks, albums, and artists that are not available for
            streaming from search results.
          </span>
        </div>
        <label class="toggle">
          <input type="checkbox" v-model="excludeUnavailable" />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="settingsSection">
      <h2 class="sectionTitle">Display</h2>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Show Images</span>
          <span class="settingDescription">
            Display album and artist images throughout the app.
          </span>
        </div>
        <label class="toggle">
          <input type="checkbox" v-model="imagesEnabled" />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="settingsSection">
      <h2 class="sectionTitle">Playback</h2>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Smart Continuation</span>
          <span class="settingDescription">
            Automatically add a related track when the current queue reaches its
            final track.
          </span>
        </div>
        <label class="toggle">
          <input type="checkbox" v-model="smartContinuationEnabled" />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useDebugStore } from "@/store/debug";
import { useUserStore } from "@/store/user";
import { storeToRefs } from "pinia";
import { computed } from "vue";

const debugStore = useDebugStore();
const { useOrganicSearch, imagesEnabled, excludeUnavailable } =
  storeToRefs(debugStore);
const userStore = useUserStore();
const smartContinuationEnabled = computed({
  get: () => userStore.isSmartContinuationEnabled,
  set: (enabled) => userStore.setSmartContinuationEnabled(enabled),
});
</script>

<style scoped>
.settingsPage {
  display: flex;
  flex-direction: column;
  gap: 24px;
  width: 100%;
  min-height: 100%;
  padding: clamp(18px, 2vw, 30px);
  color: var(--text-base);
}

.pageTitle {
  margin: 0;
  color: #9eddb7;
  font-size: clamp(1.25rem, 1.8vw, 1.65rem);
  font-weight: 900;
  line-height: 1.1;
  text-transform: uppercase;
}

.settingsSection {
  display: flex;
  flex-direction: column;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--surface-panel);
  overflow: hidden;
}

.sectionTitle {
  margin: 0;
  padding: 14px 16px;
  border-bottom: 1px solid var(--surface-border);
  color: #9eddb7;
  font-size: 0.82rem;
  font-weight: 900;
  letter-spacing: 0;
  text-transform: uppercase;
}

.settingRow {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 18px;
  padding: 16px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.055);
}

.settingRow:last-child {
  border-bottom: none;
}

.settingInfo {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}

.settingLabel {
  color: var(--text-base);
  font-size: 0.96rem;
  font-weight: 850;
}

.settingDescription {
  max-width: 680px;
  color: rgba(255, 255, 255, 0.62);
  font-size: 0.84rem;
  font-weight: 600;
  line-height: 1.35;
}

.settingSelect {
  flex-shrink: 0;
  padding: var(--spacing-2) var(--spacing-3);
  border: 1px solid var(--surface-border);
  border-radius: var(--radius-md);
  background-color: var(--surface-hover);
  color: var(--text-base);
  font-size: var(--text-sm);
  cursor: pointer;
}

.settingSelect:focus {
  outline: 2px solid var(--accent-color);
  outline-offset: 1px;
}

.toggle {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 28px;
  flex-shrink: 0;
}

.toggle input {
  width: 0;
  height: 0;
  opacity: 0;
}

.toggle-slider {
  position: absolute;
  inset: 0;
  cursor: pointer;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 28px;
  background-color: rgba(255, 255, 255, 0.09);
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast);
}

.toggle-slider::before {
  position: absolute;
  content: "";
  width: 20px;
  height: 20px;
  left: 3px;
  bottom: 3px;
  border-radius: 50%;
  background-color: var(--text-base);
  transition: transform var(--transition-fast);
}

.toggle input:checked + .toggle-slider {
  border-color: var(--spotify-green);
  background-color: var(--spotify-green);
}

.toggle input:checked + .toggle-slider::before {
  transform: translateX(20px);
  background-color: #071108;
}

.toggle input:focus-visible + .toggle-slider {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
}

@media (max-width: 720px) {
  .settingsPage {
    padding: 14px;
    gap: 18px;
  }

  .settingRow {
    grid-template-columns: 1fr;
    gap: 12px;
  }
}
</style>
