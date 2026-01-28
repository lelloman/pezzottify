<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useRemotePlaybackStore } from "@/store/remotePlayback";

const remotePlayback = useRemotePlaybackStore();

const showDropdown = ref(false);
const selectorRef = ref(null);

const currentDevice = computed(() =>
  remotePlayback.devices.find((d) => d.id === remotePlayback.deviceId),
);

const audioDevice = computed(() => remotePlayback.audioDevice);

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

function selectDevice(device) {
  if (device.is_audio_device) {
    closeDropdown();
    return; // Already audio device
  }
  if (device.id === remotePlayback.deviceId) {
    // This device wants to become audio device
    if (!remotePlayback.isAudioDevice && remotePlayback.sessionExists) {
      remotePlayback.requestBecomeAudioDevice();
    }
  }
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
      :class="{ 'remote-active': remotePlayback.isRemoteMode }"
      :title="
        audioDevice && audioDevice.id !== remotePlayback.deviceId
          ? `Playing on ${audioDevice.name}`
          : 'Connected devices'
      "
    >
      <span class="device-icon">{{ getDeviceIcon(currentDevice?.device_type) }}</span>
      <span
        v-if="audioDevice && audioDevice.id !== remotePlayback.deviceId"
        class="remote-indicator"
      >
        •
      </span>
    </button>

    <div v-if="showDropdown" class="device-dropdown">
      <div class="dropdown-header">Connected devices</div>

      <div
        v-for="device in remotePlayback.devices"
        :key="device.id"
        class="device-item"
        :class="{
          active: device.is_audio_device,
          current: device.id === remotePlayback.deviceId,
        }"
        @click="selectDevice(device)"
      >
        <span class="device-icon">{{ getDeviceIcon(device.device_type) }}</span>
        <span class="device-name">{{ device.name }}</span>
        <span v-if="device.is_audio_device" class="playing-indicator">Playing</span>
        <span v-if="device.id === remotePlayback.deviceId" class="this-device"
          >(this device)</span
        >
      </div>

      <div v-if="remotePlayback.devices.length === 0" class="no-devices">
        No devices connected
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

.device-button.remote-active {
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
