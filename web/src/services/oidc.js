import { UserManager, WebStorageStateStore } from "oidc-client-ts";

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
 */
export async function login() {
  const manager = getUserManager();
  const deviceInfo = getDeviceInfo();

  // Pass device info as extra query params
  await manager.signinRedirect({
    extraQueryParams: {
      device_id: deviceInfo.deviceId,
      device_type: deviceInfo.deviceType,
      device_name: deviceInfo.deviceName,
    },
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
 * Get the current ID token.
 * Returns null if not logged in.
 */
export async function getIdToken() {
  const user = await getUser();
  return user?.id_token || null;
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
 */
export async function refreshTokens() {
  const manager = getUserManager();
  const user = await manager.getUser();

  if (!user?.refresh_token) {
    console.debug("[OIDC] No refresh token available");
    return null;
  }

  try {
    console.debug("[OIDC] Attempting token refresh");
    const newUser = await manager.signinSilent();
    console.debug("[OIDC] Token refresh successful");
    // Update cookie with new token for WebSocket
    if (newUser?.id_token) {
      setSessionCookie(newUser.id_token);
    }
    return newUser;
  } catch (error) {
    console.error("[OIDC] Token refresh failed:", error);
    return null;
  }
}

/**
 * Logout - clear local tokens and optionally redirect to OIDC provider logout.
 */
export async function logout(redirectToProvider = false) {
  const manager = getUserManager();

  // Clear the session cookie
  clearSessionCookie();

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
};
