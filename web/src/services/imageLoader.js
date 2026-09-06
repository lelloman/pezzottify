// Fetch explicitly: native Image.onerror does not expose the HTTP status.
export function abortableDelay(ms, signal) {
  return new Promise((resolve, reject) => {
    signal.throwIfAborted();
    const onAbort = () => {
      clearTimeout(timer);
      reject(signal.reason);
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", onAbort);
      resolve();
    }, ms);
    signal.addEventListener("abort", onAbort, { once: true });
  });
}

export async function fetchImageBlob(
  url,
  {
    signal,
    fetchImpl = globalThis.fetch,
    wait = abortableDelay,
    random = Math.random,
    timeoutMs = 15_000,
  } = {},
) {
  signal ??= new AbortController().signal;
  for (let attempt = 0; ; attempt++) {
    signal.throwIfAborted();
    const controller = new AbortController();
    const onAbort = () => controller.abort(signal.reason);
    signal.addEventListener("abort", onAbort, { once: true });
    let timedOut = false;
    const timer = setTimeout(() => {
      timedOut = true;
      controller.abort();
    }, timeoutMs);
    let retryable = false;
    try {
      const response = await fetchImpl(url, {
        signal: controller.signal,
        credentials: "same-origin",
      });
      if (!response.ok) {
        retryable =
          (response.status >= 500 && response.status <= 599) ||
          response.status === 408;
        // Release the response body before retrying.
        await response.body?.cancel();
        throw new Error(`Image request failed: HTTP ${response.status}`);
      }
      return await response.blob();
    } catch (error) {
      signal.throwIfAborted();
      // Fetch reports network failures as TypeError; our own deadline is retryable too.
      if (
        attempt >= 3 ||
        !(retryable || timedOut || error instanceof TypeError)
      ) {
        throw error;
      }
    } finally {
      clearTimeout(timer);
      signal.removeEventListener("abort", onAbort);
    }
    // Equal jitter: 250–500ms, 500–1000ms, then 1000–2000ms.
    const ceiling = 500 * 2 ** attempt;
    await wait(ceiling / 2 + (random() * ceiling) / 2, signal);
  }
}
