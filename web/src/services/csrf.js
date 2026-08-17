function readCookie(name) {
  const prefix = `${encodeURIComponent(name)}=`;
  return document.cookie
    .split(";")
    .map((cookie) => cookie.trim())
    .find((cookie) => cookie.startsWith(prefix))
    ?.slice(prefix.length);
}

export function getCsrfToken() {
  const token =
    readCookie("__Host-csrf_token") || readCookie("csrf_token");
  return token ? decodeURIComponent(token) : null;
}

export function withCsrfHeader(headers = {}) {
  const token = getCsrfToken();
  return token ? { ...headers, "X-CSRF-Token": token } : headers;
}
