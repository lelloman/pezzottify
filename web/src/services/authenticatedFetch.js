import { authorizationHeaderValue } from "./authorization.js";
import { getCsrfToken } from "./csrf.js";

// Keep native streaming responses while applying the same credentials as axios.
export function createAuthenticatedFetch(oidc, fetchImpl = fetch) {
  return async (url, options = {}) => {
    options.signal?.throwIfAborted();
    const headers = new Headers(options.headers);
    if (
      !["GET", "HEAD", "OPTIONS"].includes(
        (options.method || "GET").toUpperCase(),
      )
    ) {
      const csrf = getCsrfToken();
      if (csrf) headers.set("X-CSRF-Token", csrf);
    }
    const token = await oidc.getIdToken();
    options.signal?.throwIfAborted();
    if (token) headers.set("Authorization", authorizationHeaderValue(token));
    const response = await fetchImpl(url, { ...options, headers });
    if (response.status !== 401) return response;

    // OIDC coalesces concurrent renewals. Temporary failures retain the session.
    const user = await oidc.refreshTokens();
    options.signal?.throwIfAborted();
    if (!user?.id_token) return response;
    await response.body?.cancel();
    headers.set("Authorization", authorizationHeaderValue(user.id_token));
    return fetchImpl(url, { ...options, headers });
  };
}

let authenticatedRequest;
export async function authenticatedFetch(url, options) {
  if (!authenticatedRequest) {
    const oidc = await import("./oidc.js");
    authenticatedRequest = createAuthenticatedFetch(oidc);
  }
  return authenticatedRequest(url, options);
}
