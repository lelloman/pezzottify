// One request owns the pending player state. Superseded responses never commit.
export function createRadioCreation({ onState, timeoutMs = 60000 }) {
  let active = null;
  let retryRequest = null;
  const cancel = () => {
    active?.abort();
    active = null;
    retryRequest = null;
    onState({ status: "idle" });
  };
  const start = async (request, commit) => {
    cancel();
    retryRequest = () => start(request, commit);
    const controller = new AbortController();
    active = controller;
    onState({ status: "creating" });
    let timer;
    try {
      const result = await Promise.race([
        request(controller.signal),
        new Promise((_, reject) => {
          controller.signal.addEventListener(
            "abort",
            () => reject(new Error("cancelled")),
            { once: true },
          );
          timer = setTimeout(() => reject(new Error("timeout")), timeoutMs);
        }),
      ]);
      if (active !== controller) return [];
      if (!result.trackIds.length) {
        onState({
          status: "error",
          message:
            "No playable tracks found for this radio. Try another seed or adjust the filters.",
        });
        return [];
      }
      active = null;
      onState({ status: "idle" });
      commit(result);
      retryRequest = null;
      return result.trackIds;
    } catch (error) {
      if (active !== controller) return [];
      onState({
        status: "error",
        message:
          error.message === "timeout" || error.code === "ECONNABORTED"
            ? "Radio creation timed out. Please try again."
            : "Could not create radio. Please try again.",
      });
      return [];
    } finally {
      clearTimeout(timer);
      if (active === controller) active = null;
      controller.abort();
    }
  };
  return { start, cancel, retry: () => retryRequest?.() };
}
