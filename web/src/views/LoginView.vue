<script setup>
import { ref } from 'vue';
import { useAuthStore } from '@/store/auth.js';
import { useRouter } from 'vue-router';

const authStore = useAuthStore();
const router = useRouter();
const username = ref('');
const password = ref('');
const error = ref('');
const isLoading = ref(false);

async function handleLogin() {
  if (!username.value || !password.value) {
    error.value = 'Please enter both username and password';
    return;
  }

  isLoading.value = true;
  error.value = '';

  try {
    await authStore.login({ username: username.value, password: password.value });
    router.push('/');
  } catch (e) {
    error.value = 'Invalid username or password';
  } finally {
    isLoading.value = false;
  }
}
</script>

<template>
  <div class="login-container">
    <div class="login-card">
      <div class="login-header">
        <h1 class="app-title">Pezzottify</h1>
        <p class="login-subtitle">Sign in to continue</p>
      </div>

      <form @submit.prevent="handleLogin" class="login-form">
        <div class="form-group">
          <input
            id="username"
            v-model="username"
            type="text"
            placeholder="Enter your username"
            :disabled="isLoading"
            autocomplete="username"
          />
        </div>

        <div class="form-group">
          <input
            id="password"
            type="password"
            v-model="password"
            placeholder="Enter your password"
            :disabled="isLoading"
            autocomplete="current-password"
          />
        </div>

        <div v-if="error" class="error-message">
          {{ error }}
        </div>

        <button type="submit" class="login-button" :disabled="isLoading">
          <span v-if="!isLoading">Sign In</span>
          <span v-else>Signing in...</span>
        </button>
      </form>
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

.form-group {
  display: flex;
  flex-direction: column;
}

.form-group input {
  padding: 12px 16px;
  font-size: 14px;
  background: var(--background);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  color: var(--vt-c-white);
  transition: all 0.2s ease;
  outline: none;
  font-family: inherit;
}

.form-group input::placeholder {
  color: var(--text-subdued);
}

.form-group input:focus {
  border-color: var(--accent-color);
  background: var(--highlighted-panel-color);
}

.form-group input:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.error-message {
  padding: 12px 16px;
  background: rgba(244, 67, 54, 0.1);
  border: 1px solid rgba(244, 67, 54, 0.3);
  border-radius: 6px;
  color: #ff6b6b;
  font-size: 13px;
  text-align: center;
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
