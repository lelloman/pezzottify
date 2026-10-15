// Wake before starting an application's own request timeout. Never replay that
// request: in particular, a failed mutation may already have committed.
export function createPowerClient(fetcher, now = Date.now, onReady = () => {}) {
  let pending = null;
  let readyUntil = 0;
  let legacyUntil = 0;

  async function status() {
    if (now() < legacyUntil) return "ready";
    const response = await fetcher("/_power/status", {
      credentials: "same-origin", cache: "no-store",
      signal: AbortSignal.timeout(5000),
    });
    // Rolling client deployment also works against the previous server, which
    // serves either a 404 or the SPA at this path.
    if (response.status === 404 || (response.ok && !response.headers.get("content-type")?.includes("application/json"))) {
      legacyUntil = now() + 60000;
      return "ready";
    }
    if (!response.ok) throw new Error("Server availability check failed");
    const value = await response.json();
    if (!["ready", "sleeping"].includes(value.state)) throw new Error("Invalid server availability response");
    return value.state;
  }

  function ensureReady() {
    if (now() < readyUntil) return Promise.resolve();
    if (pending) return pending;
    pending = (async () => {
      if (await status() !== "ready") {
        const response = await fetcher("/_power/wake", {
          method: "POST", credentials: "same-origin", cache: "no-store",
          signal: AbortSignal.timeout(125000),
        });
        if (!response.ok || (await response.json()).state !== "ready") {
          throw new Error("Server is still waking; try again shortly");
        }
      }
      readyUntil = now() + 2000;
      onReady();
    })().finally(() => { pending = null; });
    return pending;
  }

  return { status, ensureReady };
}

const readyListeners = new Set();
const client = createPowerClient((...args) => fetch(...args), Date.now, () => {
  for (const listener of readyListeners) listener();
});
export function onServerReady(listener) {
  readyListeners.add(listener);
  return () => readyListeners.delete(listener);
}
let lastInteraction = Date.now();
let playbackActive = () => false;

export function setPlaybackActivityProvider(provider) { playbackActive = provider; }
export function isPassiveRequest() {
  return !playbackActive() && (typeof document !== "undefined" && document.hidden || Date.now() - lastInteraction > 30000);
}
export const ensureReady = client.ensureReady;
export const powerStatus = client.status;

if (typeof window !== "undefined") {
  for (const event of ["pointerdown", "keydown", "touchstart", "wheel"]) {
    window.addEventListener(event, () => { lastInteraction = Date.now(); }, { passive: true });
  }
  window.addEventListener("focus", () => { lastInteraction = Date.now(); });
}
