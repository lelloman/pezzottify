import { defineStore } from 'pinia';
import axios from 'axios';
import * as ws from '../services/websocket';

const DEVICE_UUID_KEY = 'pezzottify_device_uuid';

function getOrCreateDeviceUuid() {
  let deviceUuid = localStorage.getItem(DEVICE_UUID_KEY);
  if (!deviceUuid) {
    // Generate a UUID-like string
    deviceUuid = 'web-' + crypto.randomUUID();
    localStorage.setItem(DEVICE_UUID_KEY, deviceUuid);
  }
  return deviceUuid;
}

function getDeviceName() {
  // Try to get a meaningful device name from the browser
  const ua = navigator.userAgent;
  if (ua.includes('Chrome')) return 'Chrome Browser';
  if (ua.includes('Firefox')) return 'Firefox Browser';
  if (ua.includes('Safari')) return 'Safari Browser';
  if (ua.includes('Edge')) return 'Edge Browser';
  return 'Web Browser';
}

function getOsInfo() {
  const ua = navigator.userAgent;
  if (ua.includes('Windows')) return 'Windows';
  if (ua.includes('Mac OS')) return 'macOS';
  if (ua.includes('Linux')) return 'Linux';
  if (ua.includes('Android')) return 'Android';
  if (ua.includes('iOS')) return 'iOS';
  return navigator.platform || 'Unknown';
}

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
          device_uuid: getOrCreateDeviceUuid(),
          device_type: 'web',
          device_name: getDeviceName(),
          os_info: getOsInfo(),
        });

        // Assuming the response contains the token in response.data.token
        this.token = response.data.token;
        localStorage.setItem('token', this.token);

        // Optionally fetch and store user info
        this.user = response.data.user || null;

        // Connect to WebSocket after successful login
        ws.connect();
      } catch (error) {
        console.error('Login failed', error);
        throw new Error(error.response?.data?.message || 'Login failed');
      }
    },
    logout() {
      // Disconnect WebSocket before clearing auth
      ws.disconnect();

      this.token = null;
      this.user = null;
      localStorage.removeItem('token');
    },
    /**
     * Initialize the auth store on app startup.
     * Connects to WebSocket if already authenticated.
     */
    initialize() {
      if (this.isAuthenticated) {
        ws.connect();
      }
    }
  }
});
