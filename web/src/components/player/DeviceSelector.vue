<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useRemotePlaybackStore } from "@/store/remotePlayback";

const remotePlayback = useRemotePlaybackStore();

const showDropdown = ref(false);
const selectorRef = ref(null);

// Current device (this device)
const thisDevice = computed(() =>
  remotePlayback.devices.find((d) => d.id === remotePlayback.deviceId),
);

// Currently selected output device
const currentOutputDevice = computed(() => remotePlayback.currentOutputDevice);

// Is output going to a remote device?
const isRemoteOutput = computed(() => !remotePlayback.isLocalOutput);

// Other devices available as output options
const otherDevices = computed(() => remotePlayback.otherDevices);

function toggleDropdown() {
  showDropdown.value = !showDropdown.value;
}

function closeDropdown() {
  showDropdown.value = false;
}

function handleClickOutside(event) {
  if (selectorRef.value && !selectorRef.value.contains(event.target)) {
    closeDropdown();
  }
}

function selectOutput(deviceId) {
  remotePlayback.selectOutputDevice(deviceId);
  closeDropdown();
}

function selectThisDevice() {
  remotePlayback.selectOutputDevice(null);
  closeDropdown();
}

function getDeviceIcon(type) {
  switch (type) {
    case "web":
      return "🖥️";
    case "android":
      return "📱";
    case "ios":
      return "📱";
    default:
      return "💻";
  }
}

onMounted(() => {
  document.addEventListener("click", handleClickOutside);
});

onUnmounted(() => {
  document.removeEventListener("click", handleClickOutside);
});
</script>

<template>
  <div class="device-selector" ref="selectorRef">
    <button
      v-if="remotePlayback.devices.length > 0"
      class="device-button"
      @click="toggleDropdown"
      :class="{ 'remote-output': isRemoteOutput }"
      :title="
        isRemoteOutput
          ? `Playing on ${currentOutputDevice?.name}`
          : 'Select output device'
      "
    >
      <span class="device-icon">{{ getDeviceIcon(currentOutputDevice?.device_type) }}</span>
      <span v-if="isRemoteOutput" class="remote-indicator"> • </span>
    </button>

    <div v-if="showDropdown" class="device-dropdown">
      <div class="dropdown-header">Select output device</div>

      <!-- This device option -->
      <div
        v-if="thisDevice"
        class="device-item"
        :class="{ active: remotePlayback.isLocalOutput }"
        @click="selectThisDevice"
      >
        <span class="device-icon">{{ getDeviceIcon(thisDevice.device_type) }}</span>
        <span class="device-name">{{ thisDevice.name }}</span>
        <span v-if="remotePlayback.isLocalOutput" class="playing-indicator">Playing</span>
        <span class="this-device">(this device)</span>
      </div>

      <!-- Other devices -->
      <div
        v-for="device in otherDevices"
        :key="device.id"
        class="device-item"
        :class="{ active: remotePlayback.selectedOutputDevice === device.id }"
        @click="selectOutput(device.id)"
      >
        <span class="device-icon">{{ getDeviceIcon(device.device_type) }}</span>
        <span class="device-name">{{ device.name }}</span>
        <span
          v-if="remotePlayback.selectedOutputDevice === device.id"
          class="playing-indicator"
          >Playing</span
        >
      </div>

      <div v-if="remotePlayback.devices.length <= 1" class="no-devices">
        No other devices connected
      </div>
    </div>
  </div>
</template>

<style scoped>
.device-selector {
  position: relative;
}

.device-button {
  display: flex;
  align-items: center;
  gap: var(--spacing-1);
  padding: var(--spacing-1) var(--spacing-2);
  background: transparent;
  border: none;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text-subdued);
  transition: all var(--transition-fast);
}

.device-button:hover {
  background: var(--bg-highlight);
  color: var(--text-base);
}

.device-button.remote-output {
  color: var(--spotify-green);
}

.device-icon {
  font-size: 16px;
}

.remote-indicator {
  color: var(--spotify-green);
  font-size: 20px;
  line-height: 1;
}

.device-dropdown {
  position: absolute;
  bottom: 100%;
  right: 0;
  margin-bottom: var(--spacing-2);
  min-width: 280px;
  max-width: 320px;
  background: var(--bg-elevated);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-lg);
  overflow: hidden;
  z-index: 1000;
}

.dropdown-header {
  padding: var(--spacing-3);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  border-bottom: 1px solid var(--border-default);
}

.device-item {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-3);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.device-item:hover {
  background: var(--bg-highlight);
}

.device-item.active {
  background: var(--bg-tinted);
}

.device-item.active .device-name {
  color: var(--spotify-green);
}

.device-name {
  flex: 1;
  font-size: var(--text-sm);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.playing-indicator {
  font-size: var(--text-xs);
  color: var(--spotify-green);
  background: rgba(30, 215, 96, 0.1);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.this-device {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.no-devices {
  padding: var(--spacing-4);
  text-align: center;
  color: var(--text-subdued);
  font-size: var(--text-sm);
}
</style>
