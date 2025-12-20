<script setup>
import { ref } from "vue";
import { useAuthStore } from "@/store/auth.js";

const authStore = useAuthStore();
const isLoading = ref(false);

function handleLogin() {
  isLoading.value = true;
  authStore.loginWithOidc();
}
</script>

<template>
  <div class="login-container">
    <div class="login-card">
      <div class="login-header">
        <h1 class="app-title">Pezzottify</h1>
        <p class="login-subtitle">Sign in to continue</p>
      </div>

      <div class="login-form">
        <button
          type="button"
          class="login-button"
          :disabled="isLoading"
          @click="handleLogin"
        >
          <span v-if="!isLoading">Sign in with LelloAuth</span>
          <span v-else>Redirecting...</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.login-container {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 100vh;
  width: 100vw;
  background: var(--background);
  padding: 20px;
}

.login-card {
  background: var(--panel-on-bg);
  border-radius: var(--panel-border-radius);
  padding: 48px 40px;
  width: 100%;
  max-width: 420px;
  box-shadow: 0 4px 6px rgba(0, 0, 0, 0.3);
}

.login-header {
  text-align: center;
  margin-bottom: 36px;
}

.app-title {
  font-size: 32px;
  font-weight: 600;
  color: var(--accent-color);
  margin: 0 0 8px 0;
  letter-spacing: -0.5px;
}

.login-subtitle {
  font-size: 14px;
  color: var(--color-text);
  margin: 0;
  opacity: 0.7;
}

.login-form {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.login-button {
  padding: 14px 24px;
  font-size: 15px;
  font-weight: 600;
  color: white;
  background: var(--accent-color);
  border: none;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s ease;
  margin-top: 8px;
  font-family: inherit;
}

.login-button:hover:not(:disabled) {
  background: rgb(255, 100, 10);
  transform: translateY(-1px);
  box-shadow: 0 4px 12px rgba(243, 89, 0, 0.3);
}

.login-button:active:not(:disabled) {
  transform: translateY(0);
}

.login-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

@media (max-width: 480px) {
  .login-card {
    padding: 36px 24px;
  }

  .app-title {
    font-size: 28px;
  }
}
</style>
