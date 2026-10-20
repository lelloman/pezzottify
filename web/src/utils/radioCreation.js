// One request owns the pending player state. Superseded responses never commit.
//
// The pending state is `null` when idle, otherwise
// `{ status: "creating" | "error", message?, context }`, where `context`
// describes what is being created (known before the request starts) and can be
// refined while the request runs (e.g. once the seed label resolves).
export function createRadioCreation({ onState, timeoutMs = 60000 }) {
  let active = null; // controller of the in-flight request
  let owner = null; // controller of the attempt that owns the pending state
  let pending = null;
  let retryRequest = null;

  const emit = (next) => {
    pending = next;
    onState(next);
  };

  const cancel = () => {
    active?.abort();
    active = null;
    owner = null;
    retryRequest = null;
    emit(null);
  };

  const start = async (context, request, commit) => {
    cancel();
    const controller = new AbortController();
    active = controller;
    owner = controller;
    let latestContext = { ...context };
    retryRequest = () => start(latestContext, request, commit);
    const updateContext = (patch) => {
      if (owner !== controller || !pending) return;
      latestContext = { ...latestContext, ...patch };
      emit({ ...pending, context: latestContext });
    };
    const fail = (message) => {
      if (owner !== controller) return;
      emit({ status: "error", message, context: latestContext });
    };
    emit({ status: "creating", context: latestContext });
    let timer;
    try {
      const result = await Promise.race([
        request(controller.signal, { context: latestContext, updateContext }),
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
        fail(
          "No playable tracks found for this radio. Try another seed or adjust the filters.",
        );
        return [];
      }
      active = null;
      owner = null;
      emit(null);
      commit(result);
      retryRequest = null;
      return result.trackIds;
    } catch (error) {
      if (active !== controller) return [];
      fail(
        error.message === "timeout" || error.code === "ECONNABORTED"
          ? "Radio creation timed out. Please try again."
          : "Could not create radio. Please try again.",
      );
      return [];
    } finally {
      clearTimeout(timer);
      if (active === controller) active = null;
      controller.abort();
    }
  };

  return { start, cancel, retry: () => retryRequest?.() };
}
