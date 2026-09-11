let authToken = null;

self.addEventListener("message", (event) => {
  const data = event?.data;
  if (!data || data.type !== "SET_AUTH_TOKEN") return;
  authToken = data.token || null;
});

function shouldAttachAuth(request) {
  if (request.method !== "GET") return false;
  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return false;
  return (
    url.pathname.startsWith("/v1/content/stream/") ||
    url.pathname.startsWith("/v1/content/image/")
  );
}

// The worker can be terminated and restarted at any time, which drops the
// in-memory token. Ask a page for a fresh one so media requests still work.
async function requestTokenFromClients() {
  const clientList = await self.clients.matchAll({
    type: "window",
    includeUncontrolled: true,
  });

  for (const client of clientList) {
    const token = await new Promise((resolve) => {
      const channel = new MessageChannel();
      const timeoutId = setTimeout(() => resolve(null), 5000);
      channel.port1.onmessage = (event) => {
        clearTimeout(timeoutId);
        resolve(event.data?.token || null);
      };
      try {
        client.postMessage({ type: "GET_AUTH_TOKEN" }, [channel.port2]);
      } catch {
        clearTimeout(timeoutId);
        resolve(null);
      }
    });
    if (token) return token;
  }
  return null;
}

async function fetchWithAuth(request) {
  let token = authToken;
  if (!token) {
    token = await requestTokenFromClients();
    if (token) authToken = token;
  }
  if (!token) return fetch(request);

  const headers = new Headers(request.headers);
  if (!headers.has("Authorization")) {
    headers.set("Authorization", token);
  }
  const response = await fetch(new Request(request, { headers }));
  if (response.status !== 401) return response;

  // The cached token may be stale - ask the page for a refreshed one once.
  const freshToken = await requestTokenFromClients();
  if (!freshToken || freshToken === token) return response;
  authToken = freshToken;

  const retryHeaders = new Headers(request.headers);
  retryHeaders.set("Authorization", freshToken);
  return fetch(new Request(request, { headers: retryHeaders }));
}

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (!shouldAttachAuth(request)) return;
  event.respondWith(fetchWithAuth(request));
});
