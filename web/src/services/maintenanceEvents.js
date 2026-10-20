const listeners = new Set();

export function onMaintenance(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function notifyMaintenance(state) {
  for (const listener of listeners) {
    listener(state);
  }
}
