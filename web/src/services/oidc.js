import { UserManager, WebStorageStateStore } from "oidc-client-ts";

// Track in-flight refresh to coalesce concurrent requests
let inFlightRefresh = null;

// Track when the last successful refresh completed (to debounce rapid refreshes)
let lastRefreshTime = 0;
const REFRESH_DEBOUNCE_MS = 5000; // Don't refresh again within 5 seconds of a successful refresh

// Track rate limiting backoff
let rateLimitedUntil = 0;

// Validate required OIDC configuration
const authority = import.meta.env.VITE_OIDC_AUTHORITY;
const clientId = import.meta.env.VITE_OIDC_CLIENT_ID;

if (!authority || !clientId) {
  console.error(
    "[OIDC] Missing required configuration. " +
    "Please set VITE_OIDC_AUTHORITY and VITE_OIDC_CLIENT_ID in .env.local. " +
    "See .env.example for reference."
  );
}

// OIDC Configuration - loaded from environment variables
// See .env.example for configuration options
const OIDC_CONFIG = {
  authority: authority,
  client_id: clientId,
  redirect_uri:
    import.meta.env.VITE_OIDC_REDIRECT_URI ||
    `${window.location.origin}/auth/callback`,
  post_logout_redirect_uri:
    import.meta.env.VITE_OIDC_POST_LOGOUT_REDIRECT_URI ||
    window.location.origin,
  scope: import.meta.env.VITE_OIDC_SCOPE || "openid profile email offline_access",
  response_type: "code",
  automaticSilentRenew: false, // We handle refresh manually on 401
  userStore: new WebStorageStateStore({ store: window.localStorage }),
};

let userManager = null;

/**
 * Get or create the UserManager instance.
 */
function getUserManager() {
  if (!userManager) {
    userManager = new UserManager(OIDC_CONFIG);

    // Log events for debugging
    userManager.events.addUserLoaded((user) => {
      console.debug("[OIDC] User loaded:", user?.profile?.preferred_username);
    });

    userManager.events.addUserUnloaded(() => {
      console.debug("[OIDC] User unloaded");
    });

    userManager.events.addSilentRenewError((error) => {
      console.error("[OIDC] Silent renew error:", error);
    });

    userManager.events.addAccessTokenExpiring(() => {
      console.debug("[OIDC] Access token expiring");
    });

    userManager.events.addAccessTokenExpired(() => {
      console.debug("[OIDC] Access token expired");
    });
  }
  return userManager;
}

/**
 * Get device info for OIDC login (similar to Android).
 */
function getDeviceInfo() {
  const deviceId = getOrCreateDeviceId();
  const deviceType = "web";
  const deviceName = getDeviceName();
  return { deviceId, deviceType, deviceName };
}

/**
 * Get or create a persistent device ID.
 */
function getOrCreateDeviceId() {
  const DEVICE_ID_KEY = "pezzottify_device_id";
  let deviceId = localStorage.getItem(DEVICE_ID_KEY);
  if (!deviceId) {
    deviceId = crypto.randomUUID();
    localStorage.setItem(DEVICE_ID_KEY, deviceId);
  }
  return deviceId;
}

const LAST_USERNAME_KEY = "pezzottify_last_username";

/**
 * Store the last used username for login hint.
 * @param {string} username - The username to store
 */
export function storeLastUsername(username) {
  if (username) {
    localStorage.setItem(LAST_USERNAME_KEY, username);
    console.debug("[OIDC] Stored last username:", username);
  }
}

/**
 * Get the last used username for login hint.
 * @returns {string|null} The last username or null
 */
export function getLastUsername() {
  return localStorage.getItem(LAST_USERNAME_KEY);
}

/**
 * Set the session cookie with the ID token.
 * This is needed for WebSocket connections since browsers can't send custom headers.
 */
function setSessionCookie(idToken) {
  // Set cookie with SameSite=Lax for security (matches backend cookie settings)
  document.cookie = `session_token=${idToken}; path=/; SameSite=Lax`;
  console.debug("[OIDC] Session cookie set");
}

/**
 * Clear the session cookie.
 */
function clearSessionCookie() {
  document.cookie = "session_token=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
  console.debug("[OIDC] Session cookie cleared");
}

/**
 * Get a human-readable device name based on browser/platform info.
 */
function getDeviceName() {
  const userAgent = navigator.userAgent;
  let browser = "Browser";
  let os = "Unknown";

  if (userAgent.includes("Edg/")) browser = "Edge";
  else if (userAgent.includes("Firefox")) browser = "Firefox";
  else if (userAgent.includes("Chrome")) browser = "Chrome";
  else if (userAgent.includes("Safari")) browser = "Safari";

  if (userAgent.includes("Windows")) os = "Windows";
  else if (userAgent.includes("Mac")) os = "macOS";
  else if (userAgent.includes("Linux")) os = "Linux";
  else if (userAgent.includes("Android")) os = "Android";
  else if (userAgent.includes("iPhone") || userAgent.includes("iPad"))
    os = "iOS";

  return `${browser} on ${os}`;
}

/**
 * Initiate OIDC login flow.
 * Redirects the browser to the OIDC provider.
 *
 * @param {string} [loginHint] - Optional username hint to pre-fill on login page
 */
export async function login(loginHint = null) {
  const manager = getUserManager();
  const deviceInfo = getDeviceInfo();

  const extraQueryParams = {
    device_id: deviceInfo.deviceId,
    device_type: deviceInfo.deviceType,
    device_name: deviceInfo.deviceName,
  };

  // Add login_hint if provided (standard OIDC parameter)
  if (loginHint) {
    extraQueryParams.login_hint = loginHint;
  }

  await manager.signinRedirect({
    extraQueryParams,
  });
}

/**
 * Handle the OIDC callback after redirect from provider.
 * Returns the user object if successful.
 */
export async function handleCallback() {
  const manager = getUserManager();
  try {
    const user = await manager.signinRedirectCallback();
    console.debug("[OIDC] Callback handled successfully");
    // Set cookie for WebSocket auth (browsers can't send custom headers on WebSocket)
    if (user?.id_token) {
      setSessionCookie(user.id_token);
    }
    return user;
  } catch (error) {
    console.error("[OIDC] Callback error:", error);
    throw error;
  }
}

/**
 * Get the current user from storage.
 * Returns null if not logged in.
 * Also ensures the session cookie is set for WebSocket connections.
 */
export async function getUser() {
  const manager = getUserManager();
  const user = await manager.getUser();
  // Ensure cookie is set if we have a valid user (for WebSocket)
  if (user?.id_token && !user.expired) {
    setSessionCookie(user.id_token);
  }
  return user;
}

/**
 * Get the current ID token, refreshing if expired.
 * Returns null if not logged in or refresh fails.
 */
export async function getIdToken() {
  const manager = getUserManager();
  let user = await manager.getUser();

  if (!user) {
    return null;
  }

  // If token is expired or about to expire (within 30 seconds), refresh it
  if (user.expired || (user.expires_at && user.expires_at - Date.now() / 1000 < 30)) {
    console.debug("[OIDC] Token expired or expiring soon, refreshing...");
    user = await refreshTokens();
    if (!user) {
      console.debug("[OIDC] Token refresh failed, returning null");
      return null;
    }
  }

  return user.id_token || null;
}

/**
 * Get the current access token.
 * Returns null if not logged in.
 */
export async function getAccessToken() {
  const user = await getUser();
  return user?.access_token || null;
}

/**
 * Check if the user is logged in (has valid tokens).
 */
export async function isLoggedIn() {
  const user = await getUser();
  return user !== null && !user.expired;
}

/**
 * Refresh tokens using the refresh token.
 * Returns the new user object if successful, null if refresh fails.
 *
 * This function coalesces concurrent refresh requests - multiple callers
 * will share the same OIDC refresh call to prevent rate limiting.
 */
export async function refreshTokens() {
  const now = Date.now();

  // Check if we're currently rate limited
  if (rateLimitedUntil > now) {
    const remainingMs = rateLimitedUntil - now;
    console.debug(`[OIDC] Rate limited, ${remainingMs}ms remaining`);
    return null;
  }

  // If we just refreshed successfully, return current user instead of refreshing again
  // This prevents rapid successive refreshes when multiple 401s arrive simultaneously
  if (lastRefreshTime > 0 && now - lastRefreshTime < REFRESH_DEBOUNCE_MS) {
    console.debug("[OIDC] Recently refreshed, returning current user");
    const manager = getUserManager();
    const user = await manager.getUser();
    if (user && !user.expired) {
      persistLog("OIDC_REFRESH", { debounced: true, msSinceLastRefresh: now - lastRefreshTime });
      return user;
    }
    // If user is still expired/invalid, fall through to refresh
  }

  // If there's already an in-flight refresh, join it
  if (inFlightRefresh) {
    console.debug("[OIDC] Joining existing in-flight refresh");
    return inFlightRefresh;
  }

  // We're the first - create a new refresh promise
  console.debug("[OIDC] Starting new token refresh");
  inFlightRefresh = performRefresh();

  try {
    const result = await inFlightRefresh;
    if (result) {
      lastRefreshTime = Date.now();
    }
    return result;
  } finally {
    // Clear in-flight refresh so subsequent calls start fresh
    inFlightRefresh = null;
  }
}

/**
 * Actually perform the token refresh (internal function).
 * This is called only once even when multiple concurrent requests need refresh.
 */
async function performRefresh() {
  const manager = getUserManager();
  const user = await manager.getUser();

  if (!user?.refresh_token) {
    console.debug("[OIDC] No refresh token available");
    persistLog("OIDC_REFRESH", { error: "No refresh token available" });
    return null;
  }

  const debugInfo = {
    refreshTokenLength: user.refresh_token.length,
    expired: user.expired,
    expiresAt: user.expires_at,
    now: Date.now() / 1000,
  };
  console.debug("[OIDC] Refresh token available, length:", user.refresh_token.length);
  console.debug("[OIDC] Current user expired:", user.expired);
  console.debug("[OIDC] Current user expires_at:", user.expires_at, "now:", Date.now() / 1000);

  try {
    console.debug("[OIDC] Attempting OIDC token refresh via signinSilent");
    const newUser = await manager.signinSilent();
    console.debug("[OIDC] Token refresh successful");
    console.debug("[OIDC] New user expires_at:", newUser?.expires_at);
    // Log whether new user has refresh token (critical for diagnosing rotation issues)
    const hasNewRefreshToken = !!newUser?.refresh_token;
    const newRefreshTokenLength = newUser?.refresh_token?.length || 0;
    console.debug("[OIDC] New user has refresh token:", hasNewRefreshToken, "length:", newRefreshTokenLength);
    persistLog("OIDC_REFRESH", {
      ...debugInfo,
      success: true,
      newExpiresAt: newUser?.expires_at,
      hasNewRefreshToken,
      newRefreshTokenLength,
    });
    // Update cookie with new token for WebSocket
    if (newUser?.id_token) {
      setSessionCookie(newUser.id_token);
    }
    return newUser;
  } catch (error) {
    // Check for rate limiting
    if (isRateLimitError(error)) {
      const backoffMs = parseRetryAfter(error) || 60000; // Default 1 minute
      rateLimitedUntil = Date.now() + backoffMs;
      console.warn(`[OIDC] Rate limited by provider, backing off for ${backoffMs}ms`);
      persistLog("OIDC_REFRESH", { ...debugInfo, error: "Rate limited", backoffMs });
    } else {
      console.error("[OIDC] Token refresh failed:", error);
      console.error("[OIDC] Error name:", error?.name);
      console.error("[OIDC] Error message:", error?.message);
      persistLog("OIDC_REFRESH", {
        ...debugInfo,
        error: error?.message || "Unknown error",
        errorName: error?.name,
        errorDetails: String(error),
      });
    }
    return null;
  }
}

/**
 * Persist log to localStorage so it survives page navigation/reload.
 * Can be retrieved later with: JSON.parse(localStorage.getItem('oidc_debug_logs'))
 */
function persistLog(event, data) {
  try {
    const logs = JSON.parse(localStorage.getItem("oidc_debug_logs") || "[]");
    logs.push({
      timestamp: new Date().toISOString(),
      event,
      ...data,
    });
    // Keep only last 50 entries
    while (logs.length > 50) logs.shift();
    localStorage.setItem("oidc_debug_logs", JSON.stringify(logs));
  } catch {
    // Ignore storage errors
  }
}

/**
 * Check if an error indicates rate limiting.
 */
function isRateLimitError(error) {
  // oidc-client-ts wraps fetch errors, check for common rate limit indicators
  if (error?.status === 429) return true;
  if (error?.statusCode === 429) return true;
  if (error?.response?.status === 429) return true;
  // Some providers return 400 with specific error codes
  const errorMessage = error?.message?.toLowerCase() || "";
  const errorBody = error?.body?.toLowerCase() || "";
  return (
    errorMessage.includes("rate") ||
    errorMessage.includes("too many") ||
    errorBody.includes("rate") ||
    errorBody.includes("too many")
  );
}

/**
 * Parse Retry-After value from error response.
 */
function parseRetryAfter(error) {
  // Try to get Retry-After header from various error formats
  const retryAfter =
    error?.headers?.get?.("retry-after") ||
    error?.response?.headers?.get?.("retry-after") ||
    error?.retryAfter;

  if (retryAfter) {
    const seconds = parseInt(retryAfter, 10);
    if (!isNaN(seconds)) {
      return seconds * 1000;
    }
  }
  return null;
}

/**
 * Logout - clear local tokens and optionally redirect to OIDC provider logout.
 */
export async function logout(redirectToProvider = false) {
  const manager = getUserManager();

  // Clear the session cookie
  clearSessionCookie();

  // Clear rate limit and debounce state so user can log in immediately
  rateLimitedUntil = 0;
  lastRefreshTime = 0;

  if (redirectToProvider) {
    await manager.signoutRedirect();
  } else {
    // Just clear local state without redirecting to provider
    await manager.removeUser();
  }
}

/**
 * Clear all OIDC-related data from storage.
 */
export async function clearStorage() {
  const manager = getUserManager();
  await manager.clearStaleState();
  await manager.removeUser();
  // Reset state
  rateLimitedUntil = 0;
  lastRefreshTime = 0;
}

export default {
  login,
  handleCallback,
  getUser,
  getIdToken,
  getAccessToken,
  isLoggedIn,
  refreshTokens,
  logout,
  clearStorage,
  storeLastUsername,
  getLastUsername,
};
