export const MAX_BYTES = 2097152;
export const bytes = (value) => new TextEncoder().encode(value).length;
const secretKey =
  /authorization|cookie|password|secret|token|api.?key|credential|headers|reasoning|system.?prompt/i;
export function redact(value) {
  if (typeof value === "string") {
    if (/^\s*[{[]/.test(value)) {
      let parsed;
      try {
        parsed = JSON.parse(value);
      } catch {
        /* plain text */
      }
      if (parsed && typeof parsed === "object")
        return JSON.stringify(redact(parsed));
    }
    return value
      .replace(/Bearer\s+[^\s"<>]+/gi, "Bearer [REDACTED]")
      .replace(
        /\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g,
        "[REDACTED]",
      )
      .replace(
        /\b(?:sk-|AIza|gh[pousr]_|github_pat_|xox[baprs]-)[A-Za-z0-9_-]{12,}/g,
        "[REDACTED]",
      );
  }
  if (Array.isArray(value)) return value.map(redact);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value).map(([k, v]) => [
        k,
        secretKey.test(k) ? "[REDACTED]" : redact(v),
      ]),
    );
  return value;
}
export function bounded(envelope, limit = MAX_BYTES) {
  const result = redact(structuredClone(envelope));
  const marker = {
    kind: "metadata",
    timestamp: result.captured_at,
    content: "[TRUNCATED: diagnostic event limit]",
  };
  for (const turn of result.turns)
    if (turn.events.length > 8192)
      turn.events = [marker, ...turn.events.slice(-8191)];
  while (result.turns.length > 4096) {
    const index = result.turns.findIndex((t) => t.status !== "running");
    if (index < 0) throw new Error("Too many running diagnostic turns");
    result.turns.splice(index, 1);
  }
  const size = () => bytes(JSON.stringify(result));
  while (size() > limit && result.turns.length > 1) {
    const index = result.turns.findIndex(
      (t, i) => i < result.turns.length - 1 && t.status !== "running",
    );
    if (index < 0) break;
    result.turns.splice(index, 1);
  }
  if (size() > limit) {
    // Keep the newest turn's identity and status, explicitly marking lost content.
    const turn = result.turns.at(-1);
    result.turns = turn
      ? [
          {
            ...turn,
            events: [
              {
                kind: "metadata",
                timestamp: result.captured_at,
                content: "[TRUNCATED: diagnostic retention limit]",
              },
            ],
          },
        ]
      : [];
  }
  if (size() > limit)
    throw new Error("Diagnostic metadata exceeds storage limit");
  return result;
}
export function indexedStorage(indexedDB = globalThis.indexedDB) {
  let opened, lease;
  async function db() {
    if (!indexedDB) throw new Error("IndexedDB unavailable");
    if (!opened)
      opened = new Promise((resolve, reject) => {
        let blocked = false;
        const request = indexedDB.open("pezzottify-diagnostics-v1", 1);
        request.onupgradeneeded = () =>
          request.result.createObjectStore("recordings");
        request.onerror = () => reject(request.error);
        request.onblocked = () => {
          blocked = true;
          reject(new Error("Diagnostic storage is blocked"));
        };
        request.onsuccess = () => {
          const connection = request.result;
          if (blocked) {
            connection.close();
            opened = null;
            return;
          }
          connection.onversionchange = () => {
            connection.close();
            opened = null;
          };
          connection.onclose = () => {
            opened = null;
          };
          resolve(connection);
        };
      }).catch((error) => {
        opened = null;
        throw error;
      });
    return opened;
  }
  async function operation(mode, action) {
    const connection = await db();
    return new Promise((resolve, reject) => {
      let tx;
      try {
        tx = connection.transaction("recordings", mode);
      } catch (error) {
        connection.close();
        opened = null;
        reject(error);
        return;
      }
      const request = action(tx.objectStore("recordings"));
      tx.oncomplete = () => resolve(request?.result);
      tx.onerror = tx.onabort = () => {
        connection.close();
        opened = null;
        reject(tx.error || new Error("Diagnostic storage failed"));
      };
    });
  }
  return {
    load: () =>
      operation("readonly", (s) => {
        const epoch = s.get("epoch");
        epoch.onsuccess = () => {
          lease = epoch.result;
        };
        return s.get("current");
      }),
    save: (value) =>
      operation("readwrite", (s) => {
        const epoch = s.get("epoch");
        epoch.onsuccess = () => {
          if (epoch.result === lease) s.put(value, "current");
          else s.transaction.abort();
        };
        return epoch;
      }),
    clear: () =>
      operation("readwrite", (s) => {
        lease = crypto.randomUUID();
        s.delete("current");
        return s.put(lease, "epoch");
      }),
  };
}
export class DiagnosticRecorder {
  constructor(
    storage = indexedStorage(),
    now = () => new Date().toISOString(),
  ) {
    this.storage = storage;
    this.now = now;
    this.generation = 0;
    this.owner = null;
    this.enabled = false;
    this.envelope = { schema_version: 1, captured_at: now(), turns: [] };
    this.writes = Promise.resolve();
    this.error = null;
  }
  enqueue(generation, action) {
    const pending = this.writes.then(async () => {
      if (generation === this.generation) {
        await action();
        this.error = null;
      }
    });
    this.writes = pending.catch(() => {
      this.error =
        "Local diagnostic storage unavailable; recording was not saved.";
    });
    return this.writes;
  }
  forget(discardOwner = false) {
    this.generation++;
    this.enabled = false;
    this.envelope = { schema_version: 1, captured_at: this.now(), turns: [] };
    if (discardOwner) this.owner = null;
  }
  async account(owner) {
    const generation = ++this.generation;
    this.owner = owner;
    this.enabled = false;
    this.envelope = { schema_version: 1, captured_at: this.now(), turns: [] };
    await this.enqueue(generation, async () => {
      const stored = await this.storage.load();
      if (generation !== this.generation) return;
      if (owner && stored?.owner === owner && stored.enabled) {
        this.enabled = true;
        this.envelope = bounded(stored.envelope);
        for (const turn of this.envelope.turns)
          if (turn.status === "running") turn.status = "interrupted";
        await this.storage.save({
          owner,
          enabled: true,
          envelope: this.envelope,
        });
      } else await this.storage.clear();
    });
  }
  clear(disable = false) {
    const generation = ++this.generation;
    if (disable) this.enabled = false;
    this.envelope = { schema_version: 1, captured_at: this.now(), turns: [] };
    return this.enqueue(generation, async () => {
      await this.storage.clear();
      if (generation === this.generation && this.enabled && this.owner)
        await this.storage.save({
          owner: this.owner,
          enabled: true,
          envelope: this.envelope,
        });
    });
  }
  async enable(value) {
    const owner = this.owner;
    const clearing = this.clear(!value);
    const generation = this.generation;
    await clearing;
    // Consent belongs only to the account and lifecycle that initiated it.
    if (generation !== this.generation || owner !== this.owner) return;
    this.enabled = !!value && !!this.owner;
    await this.persist();
  }
  persist() {
    if (!this.enabled || !this.owner) return;
    try {
      this.envelope.captured_at = this.now();
      this.envelope = bounded(
        this.envelope,
        MAX_BYTES -
          bytes(
            JSON.stringify({
              owner: this.owner,
              enabled: true,
              envelope: null,
            }),
          ) +
          4,
      );
      const value = structuredClone({
        owner: this.owner,
        enabled: true,
        envelope: this.envelope,
      });
      return this.enqueue(this.generation, () => this.storage.save(value));
    } catch {
      this.error = "Diagnostic retention failed; recording unavailable.";
    }
  }
  event(turnId, conversationId, event, status = "running") {
    if (!this.enabled || !this.owner) return;
    try {
      let turn = this.envelope.turns.find((t) => t.id === turnId);
      if (!turn) {
        turn = {
          id: turnId,
          conversation_id: conversationId,
          started_at: this.now(),
          status,
          events: [],
        };
        this.envelope.turns.push(turn);
      }
      turn.events.push(redact({ timestamp: this.now(), ...event }));
      turn.status = status;
      return this.persist();
    } catch {
      this.error = "Diagnostic recording failed.";
    }
  }
  snapshot(turnId, all = false) {
    if (!this.enabled || this.error)
      throw new Error(this.error || "Recording is disabled");
    const envelope = bounded({
      ...this.envelope,
      captured_at: this.now(),
      turns: this.envelope.turns.filter((t) => all || t.id === turnId),
    });
    if (!envelope.turns.length)
      throw new Error("No retained diagnostics for this response");
    return JSON.stringify(envelope);
  }
}
