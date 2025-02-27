<script setup>
import { ref } from 'vue';
import { useAuthStore } from '@/store/auth.js';
import { useRouter } from 'vue-router';

const authStore = useAuthStore();
const router = useRouter();
const username = ref('');
const password = ref('');
const error = ref('');

async function handleLogin() {
  try {
    await authStore.login({ username: username.value, password: password.value });
    error.value = '';
    router.push('/');
  } catch (e) {
    error.value = 'Invalid credentials';
  }
}
</script>

<template>
  <div>
    <h1>Login</h1>
    <form @submit.prevent="handleLogin">
      <input v-model="username" placeholder="Username" />
      <input type="password" v-model="password" placeholder="Password" /><br>
      <button type="submit">Login</button>
      <p v-if="error">{{ error }}</p>
    </form>
  </div>
</template>
