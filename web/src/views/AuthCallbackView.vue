<script setup>
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useAuthStore } from "@/store/auth.js";

const router = useRouter();
const authStore = useAuthStore();
const error = ref(null);
const isProcessing = ref(true);

onMounted(async () => {
  try {
    const success = await authStore.handleOidcCallback();
    if (success) {
      // Redirect to home on success
      router.replace("/");
    } else {
      error.value = "Authentication failed. Please try again.";
      isProcessing.value = false;
    }
  } catch (err) {
    console.error("Auth callback error:", err);
    error.value = err.message || "An error occurred during authentication.";
    isProcessing.value = false;
  }
});

function goToLogin() {
  router.replace("/login");
}
</script>

<template>
  <div class="callback-container">
    <div class="callback-card">
      <div v-if="isProcessing" class="processing">
        <div class="spinner"></div>
        <p>Completing sign in...</p>
      </div>
      <div v-else-if="error" class="error">
        <p class="error-title">Authentication Error</p>
        <p class="error-message">{{ error }}</p>
        <button class="retry-button" @click="goToLogin">
          Back to Login
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.callback-container {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  width: 100vw;
  background: var(--background);
  padding: 20px;
}

.callback-card {
  background: var(--panel-on-bg);
  border-radius: var(--panel-border-radius);
  padding: 48px 40px;
  width: 100%;
  max-width: 420px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
  text-align: center;
}

.processing {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 20px;
}

.processing p {
  color: var(--color-text);
  font-size: 16px;
  margin: 0;
}

.spinner {
  width: 40px;
  height: 40px;
  border: 3px solid var(--panel-on-bg);
  border-top-color: var(--accent-color);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.error-title {
  color: #ff6b6b;
  font-size: 18px;
  font-weight: 600;
  margin: 0;
}

.error-message {
  color: var(--color-text);
  opacity: 0.8;
  font-size: 14px;
  margin: 0;
}

.retry-button {
  padding: 12px 24px;
  font-size: 14px;
  font-weight: 600;
  color: white;
  background: var(--accent-color);
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
  font-family: inherit;
  margin-top: 8px;
}

.retry-button:hover {
  background: rgb(255, 100, 10);
  transform: translateY(-1px);
}
</style>
