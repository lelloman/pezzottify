import { defineStore } from 'pinia';
import axios from 'axios';

export const useAuthStore = defineStore('auth', {
  state: () => ({
    user: null,
    token: localStorage.getItem('token') || null
  }),
  getters: {
    isAuthenticated: (state) => !!state.token,
  },
  actions: {
    async login(credentials) {
      try {
        const response = await axios.post('/v1/auth/login', {
          user_handle: credentials.username,
          password: credentials.password,
        });

        // Assuming the response contains the token in response.data.token
        this.token = response.data.token;
        localStorage.setItem('token', this.token);

        // Optionally fetch and store user info
        this.user = response.data.user || null;
      } catch (error) {
        console.error('Login failed', error);
        throw new Error(error.response?.data?.message || 'Login failed');
      }
    },
    logout() {
      this.token = null;
      this.user = null;
      localStorage.removeItem('token');
    }
  }
});
