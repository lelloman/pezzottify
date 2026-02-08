<template>
  <div class="deviceSelector" ref="selectorRef">
    <div
      class="deviceSelectorButton lightControlFill scaleClickFeedback mediumIcon"
      @click.stop="toggleOpen"
      :title="buttonTitle"
    >
      <svg viewBox="0 0 24 24" fill="currentColor" width="20" height="20">
        <path
          d="M20 6H4c-1.1 0-2 .9-2 2v8c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm0 10H4V8h16v8z"
        />
        <circle
          v-if="otherDevicesCount > 0"
          cx="20"
          cy="6"
          r="4"
          fill="var(--spotify-green)"
        />
      </svg>
    </div>
    <div v-if="isOpen" class="deviceDropdown">
      <div class="dropdownHeader">
        <span class="dropdownTitle">Devices</span>
      </div>
      <div class="deviceList">
        <div
          v-for="device in devices"
          :key="device.id"
          class="deviceItem"
          :class="{ activeDevice: device.is_playing }"
        >
          <div class="deviceIcon">
            <svg
              v-if="device.device_type === 'web'"
              viewBox="0 0 24 24"
              fill="currentColor"
              width="18"
              height="18"
            >
              <path
                d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 14H4V6h16v12z"
              />
            </svg>
            <svg
              v-else
              viewBox="0 0 24 24"
              fill="currentColor"
              width="18"
              height="18"
            >
              <path
                d="M16 1H8C6.34 1 5 2.34 5 4v16c0 1.66 1.34 3 3 3h8c1.66 0 3-1.34 3-3V4c0-1.66-1.34-3-3-3zm-2 20h-4v-1h4v1zm3.25-3H6.75V4h10.5v14z"
              />
            </svg>
          </div>
          <div class="deviceInfo">
            <span class="deviceName">{{ device.name }}</span>
            <span v-if="device.id === myDeviceId" class="deviceBadge"
              >This device</span
            >
            <span v-if="device.is_playing" class="deviceBadge playing"
              >Playing</span
            >
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onBeforeUnmount } from "vue";
import { usePlaybackSessionStore } from "@/store/playbackSession";

const sessionStore = usePlaybackSessionStore();
const isOpen = ref(false);
const selectorRef = ref(null);

const devices = computed(() => sessionStore.devices);
const myDeviceId = computed(() => sessionStore.myDeviceId);
const otherDevicesCount = computed(() => sessionStore.otherDevicesCount);

const buttonTitle = computed(() => {
  const count = devices.value.length;
  if (count <= 1) return "Devices";
  return `${count} devices connected`;
});

function toggleOpen() {
  isOpen.value = !isOpen.value;
}

function handleClickOutside(e) {
  if (selectorRef.value && !selectorRef.value.contains(e.target)) {
    isOpen.value = false;
  }
}

onMounted(() => {
  document.addEventListener("click", handleClickOutside);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", handleClickOutside);
});
</script>

<style scoped>
.deviceSelector {
  position: relative;
}

.deviceSelectorButton {
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-base);
  border-radius: var(--radius-full);
  transition: all var(--transition-fast);
}

.deviceSelectorButton:hover {
  color: var(--text-bright);
}

.deviceDropdown {
  position: absolute;
  bottom: 100%;
  right: 0;
  margin-bottom: var(--spacing-2);
  min-width: 280px;
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  z-index: 100;
}

.dropdownHeader {
  padding: var(--spacing-3) var(--spacing-4);
  border-bottom: 1px solid var(--bg-elevated-highlight);
}

.dropdownTitle {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-base);
}

.deviceList {
  padding: var(--spacing-2) 0;
}

.deviceItem {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-2) var(--spacing-4);
}

.deviceItem.activeDevice {
  background-color: var(--bg-elevated-highlight);
}

.deviceIcon {
  color: var(--text-subdued);
  flex-shrink: 0;
  display: flex;
  align-items: center;
}

.activeDevice .deviceIcon {
  color: var(--spotify-green);
}

.deviceInfo {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.deviceName {
  font-size: var(--text-sm);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.activeDevice .deviceName {
  color: var(--spotify-green);
}

.deviceBadge {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.deviceBadge.playing {
  color: var(--spotify-green);
}

</style>
