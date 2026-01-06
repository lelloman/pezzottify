<script setup>
import { ref } from "vue";
import { useAuthStore } from "@/store/auth.js";
import { useRemoteStore } from "@/store/remote.js";
import { useRouter } from "vue-router";

const authStore = useAuthStore();
const remoteStore = useRemoteStore();
const router = useRouter();

const isLoading = ref(false);
const username = ref("");
const password = ref("");
const error = ref("");
const showPasswordForm = ref(true);

function handleOidcLogin() {
  isLoading.value = true;
  authStore.loginWithOidc();
}

async function handlePasswordLogin() {
  if (!username.value || !password.value) {
    error.value = "Please enter username and password";
    return;
  }

  isLoading.value = true;
  error.value = "";

  try {
    const response = await fetch("/v1/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({
        user_handle: username.value,
        password: password.value,
        device_uuid: crypto.randomUUID(),
        device_type: "web",
        device_name: navigator.userAgent.substring(0, 50),
      }),
    });

    if (response.ok || response.status === 201) {
      const data = await response.json();
      // Store token if needed
      if (data.token) {
        localStorage.setItem("auth_token", data.token);
      }
      // Set user from response
      authStore.user = {
        handle: data.user_handle,
        permissions: data.permissions || [],
      };
      authStore.sessionChecked = true;
      router.push("/");
    } else {
      const data = await response.json().catch(() => ({}));
      error.value = data.error || "Login failed";
    }
  } catch (e) {
    console.error("Login error:", e);
    error.value = "Connection error: " + e.message;
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

      <div class="login-form">
        <!-- Password Login -->
        <div v-if="showPasswordForm" class="password-form">
          <input
            v-model="username"
            type="text"
            placeholder="Username"
            class="input-field"
            :disabled="isLoading"
            @keyup.enter="handlePasswordLogin"
          />
          <input
            v-model="password"
            type="password"
            placeholder="Password"
            class="input-field"
            :disabled="isLoading"
            @keyup.enter="handlePasswordLogin"
          />
          <p v-if="error" class="error-message">{{ error }}</p>
          <button
            type="button"
            class="login-button"
            :disabled="isLoading"
            @click="handlePasswordLogin"
          >
            <span v-if="!isLoading">Sign in</span>
            <span v-else>Signing in...</span>
          </button>
        </div>

        <div class="divider">
          <span>or</span>
        </div>

        <!-- OIDC Login -->
        <button
          type="button"
          class="login-button oidc-button"
          :disabled="isLoading"
          @click="handleOidcLogin"
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
  gap: 16px;
}

.password-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.input-field {
  padding: 12px 16px;
  font-size: 15px;
  color: var(--color-text);
  background: var(--background);
  border: 1px solid var(--border-color, #333);
  border-radius: 6px;
  font-family: inherit;
  transition: border-color 0.2s ease;
}

.input-field:focus {
  outline: none;
  border-color: var(--accent-color);
}

.input-field::placeholder {
  color: var(--color-text);
  opacity: 0.5;
}

.error-message {
  color: #ff6b6b;
  font-size: 13px;
  margin: 0;
  text-align: center;
}

.divider {
  display: flex;
  align-items: center;
  gap: 12px;
  color: var(--color-text);
  opacity: 0.5;
  font-size: 13px;
}

.divider::before,
.divider::after {
  content: "";
  flex: 1;
  height: 1px;
  background: var(--border-color, #333);
}

.oidc-button {
  background: #444;
}

.oidc-button:hover:not(:disabled) {
  background: #555;
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
