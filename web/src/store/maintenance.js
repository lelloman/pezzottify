import { ref } from "vue";
import { defineStore } from "pinia";
import axios from "axios";

export const useMaintenanceStore = defineStore("maintenance", () => {
  const enabled = ref(false);
  const message = ref(null);
  const estimatedEnd = ref(null);
  const checked = ref(false);
  const error = ref(false);

  function apply(state) {
    if (!state || typeof state !== "object") return;
    enabled.value = state.enabled === true;
    message.value = state.message || null;
    estimatedEnd.value = state.estimated_end || null;
  }

  async function fetchStatus() {
    try {
      const response = await axios.get("/v1/status", { powerPassive: true });
      apply(response.data);
      error.value = false;
    } catch (err) {
      console.debug("Failed to check maintenance status:", err?.response?.status);
      error.value = true;
    } finally {
      checked.value = true;
    }
  }

  return {
    enabled,
    message,
    estimatedEnd,
    checked,
    error,
    apply,
    fetchStatus,
  };
});
