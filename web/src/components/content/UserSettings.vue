<template>
  <div class="settingsPage">
    <h1 class="pageTitle">Settings</h1>

    <div class="settingsSection">
      <h2 class="sectionTitle">Search</h2>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Organic search</span>
          <span id="useOrganicSearch-description" class="settingDescription">
            Use classic flat search results. When disabled, uses smart search
            with intelligent result grouping and enrichment.
          </span>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            role="switch"
            aria-label="Organic search"
            aria-describedby="useOrganicSearch-description"
            v-model="useOrganicSearch"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Hide unavailable content</span>
          <span id="excludeUnavailable-description" class="settingDescription">
            Hide tracks, albums, and artists that are not available for
            streaming from search results.
          </span>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            role="switch"
            aria-label="Hide unavailable content"
            aria-describedby="excludeUnavailable-description"
            v-model="excludeUnavailable"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="settingsSection">
      <h2 class="sectionTitle">Display</h2>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Show images</span>
          <span id="imagesEnabled-description" class="settingDescription">
            Display album and artist images throughout the app.
          </span>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            role="switch"
            aria-label="Show images"
            aria-describedby="imagesEnabled-description"
            v-model="imagesEnabled"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="settingsSection">
      <h2 class="sectionTitle">Playback</h2>
      <div v-if="userStore.canUseProxyStreaming" class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Stream missing tracks</span>
          <span id="proxyModeEnabled-description" class="settingDescription">
            Start playing catalog tracks immediately and save them locally as
            they download.
          </span>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            role="switch"
            aria-label="Stream missing tracks"
            aria-describedby="proxyModeEnabled-description"
            v-model="proxyModeEnabled"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
      <div class="settingRow selectRow">
        <div class="settingInfo">
          <span class="settingLabel">When editing a radio queue</span>
          <span class="settingDescription"
            >Applies to all radios, including artist greatest hits.</span
          >
        </div>
        <select
          class="settingSelect"
          v-model="keepRadioOnQueueEdit"
          aria-label="When editing a radio queue"
        >
          <option :value="true">Keep radio going</option>
          <option :value="false">Stop adding tracks</option>
        </select>
      </div>
      <div class="settingRow">
        <div class="settingInfo">
          <span class="settingLabel">Smart continuation</span>
          <span
            id="smartContinuationEnabled-description"
            class="settingDescription"
          >
            Automatically queue related tracks after ordinary queues. Radios
            manage their own continuation.
          </span>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            role="switch"
            aria-label="Smart continuation"
            aria-describedby="smartContinuationEnabled-description"
            v-model="smartContinuationEnabled"
          />
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
const keepRadioOnQueueEdit = computed({
  get: () => userStore.keepRadioOnQueueEdit,
  set: (value) => userStore.setKeepRadioOnQueueEdit(value),
});
const smartContinuationEnabled = computed({
  get: () => userStore.isSmartContinuationEnabled,
  set: (enabled) => userStore.setSmartContinuationEnabled(enabled),
});
const proxyModeEnabled = computed({
  get: () => userStore.isProxyModeEnabled,
  set: (enabled) => userStore.setProxyModeEnabled(enabled),
});
</script>

<style scoped>
.settingsPage {
  display: flex;
  flex-direction: column;
  gap: 32px;
  width: 100%;
  color: var(--text-base);
  color-scheme: dark;
}
.pageTitle {
  margin: 16px 0 8px;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
}
.settingsSection {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.sectionTitle {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 700;
  line-height: 1.4;
}
.settingRow {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 24px;
  min-height: 40px;
  padding: 4px 0;
}
.settingInfo {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 4px;
}
.settingLabel {
  color: var(--text-subdued);
  font-size: 14px;
  font-weight: 400;
  line-height: 1.5;
}
.settingDescription {
  color: var(--text-subdued);
  font-size: 12px;
  font-weight: 400;
  line-height: 1.5;
}
.settingSelect {
  width: 200px;
  max-width: 100%;
  min-height: 32px;
  padding: 6px 32px 6px 12px;
  border: 1px solid transparent;
  border-radius: 4px;
  background-color: #242424;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Cpath d='m4 6 4 4 4-4' fill='none' stroke='%23b3b3b3' stroke-width='1.5'/%3E%3C/svg%3E");
  background-position: right 10px center;
  background-size: 16px;
  background-repeat: no-repeat;
  appearance: none;
  color: var(--text-subdued);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.settingSelect:hover {
  border-color: #727272;
}
.settingSelect:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 2px;
}
.toggle {
  position: relative;
  display: inline-flex;
  align-items: center;
  width: 42px;
  height: 40px;
  cursor: pointer;
}
.toggle input {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  opacity: 0;
  margin: 0;
  cursor: pointer;
}
.toggle-slider {
  position: relative;
  width: 42px;
  height: 24px;
  pointer-events: none;
  border-radius: 999px;
  background: #535353;
  transition: background-color 150ms;
}
.toggle-slider::before {
  position: absolute;
  content: "";
  width: 20px;
  height: 20px;
  left: 2px;
  top: 2px;
  border-radius: 50%;
  background: #fff;
  transition: transform 150ms;
}
.toggle:hover .toggle-slider {
  background: #727272;
}
.toggle input:checked + .toggle-slider {
  background: var(--spotify-green);
}
.toggle:hover input:checked + .toggle-slider {
  background: var(--spotify-green-hover);
}
.toggle input:checked + .toggle-slider::before {
  transform: translateX(18px);
}
.toggle input:focus-visible + .toggle-slider {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
@container (max-width: 480px) {
  .settingRow {
    gap: 16px;
  }
  .selectRow {
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
  .settingSelect {
    width: 100%;
    min-height: 40px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .toggle-slider,
  .toggle-slider::before {
    transition: none;
  }
}
</style>
