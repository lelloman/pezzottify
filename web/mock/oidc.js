// Only resolved by the dev:mock plugin. No real identity provider or credentials.
export const setupServiceWorkerBridge = () => {};
export const getIdToken = async () => null;
export const getAccessToken = async () => null;
export const getUser = async () => null;
export const refreshTokens = async () => null;
export const isLoggedIn = async () => (await fetch("/v1/auth/session")).ok;
export const getLastUsername = () => "design-demo";
export const storeLastUsername = () => {};
export const clearStorage = async () => {};
export const logout = async () => {};
export const handleCallback = async () => ({
  profile: { preferred_username: "design-demo" },
});
export async function login() {
  await fetch("/v1/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: "{}",
  });
  location.assign("/auth/callback");
}
