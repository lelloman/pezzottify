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

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (!shouldAttachAuth(request) || !authToken) return;

  const headers = new Headers(request.headers);
  if (!headers.has("Authorization")) {
    headers.set("Authorization", authToken);
  }

  const authRequest = new Request(request, { headers });
  event.respondWith(fetch(authRequest));
});
