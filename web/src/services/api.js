import axios from "axios";
import * as oidc from "./oidc";

let isRefreshing = false;
let failedQueue = [];

/**
 * Process the queue of failed requests after token refresh.
 */
function processQueue(error, token = null) {
  failedQueue.forEach((prom) => {
    if (error) {
      prom.reject(error);
    } else {
      prom.resolve(token);
    }
  });
  failedQueue = [];
}

/**
 * Check if the request URL is an auth endpoint that should skip adding Authorization header.
 */
function shouldSkipAuthHeader(url) {
  if (!url) return true;
  return (
    url.includes("/auth/login") ||
    url.includes("/auth/logout") ||
    url.includes("/auth/callback") ||
    url.includes("/auth/oidc")
  );
}

/**
 * Check if the request URL is an auth endpoint that should skip token refresh on 401.
 * Note: /auth/session is NOT excluded here because we want to refresh expired tokens
 * when checking the session on page load. The _retry flag prevents infinite loops.
 */
function shouldSkipRefresh(url) {
  if (!url) return true;
  return (
    url.includes("/auth/login") ||
    url.includes("/auth/logout") ||
    url.includes("/auth/callback") ||
    url.includes("/auth/oidc")
  );
}

/**
 * Setup axios interceptors for authentication.
 * - Request interceptor: Adds Authorization header with ID token
 * - Response interceptor: Handles 401 errors by refreshing token and retrying
 */
export function setupAxiosInterceptors() {
  // Request interceptor: Add Authorization header
  axios.interceptors.request.use(
    async (config) => {
      // Skip auth header for certain auth endpoints
      if (shouldSkipAuthHeader(config.url)) {
        return config;
      }

      const idToken = await oidc.getIdToken();
      if (idToken) {
        config.headers.Authorization = idToken;
      }
      return config;
    },
    (error) => {
      return Promise.reject(error);
    }
  );

  // Response interceptor: Handle 401 errors
  axios.interceptors.response.use(
    (response) => response,
    async (error) => {
      const originalRequest = error.config;

      // Skip if not a 401, or if it's an auth endpoint, or if we already retried
      if (
        error.response?.status !== 401 ||
        shouldSkipRefresh(originalRequest?.url) ||
        originalRequest?._retry
      ) {
        return Promise.reject(error);
      }

      // If we're already refreshing, queue this request
      if (isRefreshing) {
        return new Promise((resolve, reject) => {
          failedQueue.push({ resolve, reject });
        })
          .then((token) => {
            originalRequest.headers.Authorization = token;
            return axios(originalRequest);
          })
          .catch((err) => {
            return Promise.reject(err);
          });
      }

      originalRequest._retry = true;
      isRefreshing = true;

      try {
        console.debug("[API] Received 401, attempting token refresh");
        const newUser = await oidc.refreshTokens();

        if (newUser) {
          const newToken = newUser.id_token;
          console.debug("[API] Token refresh successful, retrying request");

          // Process queued requests
          processQueue(null, newToken);

          // Retry the original request with new token
          originalRequest.headers.Authorization = newToken;
          return axios(originalRequest);
        } else {
          // Refresh failed, clear tokens and redirect to login
          console.debug("[API] Token refresh failed, redirecting to login");
          processQueue(new Error("Token refresh failed"), null);
          await handleAuthFailure();
          return Promise.reject(error);
        }
      } catch (refreshError) {
        console.error("[API] Token refresh error:", refreshError);
        processQueue(refreshError, null);
        await handleAuthFailure();
        return Promise.reject(refreshError);
      } finally {
        isRefreshing = false;
      }
    }
  );
}

/**
 * Handle authentication failure by clearing tokens and redirecting to login.
 */
async function handleAuthFailure() {
  await oidc.logout(false);
  // Redirect to login page
  if (window.location.pathname !== "/login") {
    window.location.href = "/login";
  }
}

export default {
  setupAxiosInterceptors,
};
