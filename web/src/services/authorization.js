export function authorizationHeaderValue(token) {
  if (typeof token !== "string" || token.trim().length === 0) {
    throw new TypeError("Authorization token must not be empty");
  }
  return `Bearer ${token}`;
}
