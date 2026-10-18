import { defineStore } from "pinia";
import { ref, shallowRef } from "vue";
import {
  DiagnosticRecorder,
  bytes,
  redact,
} from "../services/feedback/diagnostics";
import * as oidc from "../services/oidc";
import { getCsrfToken } from "../services/csrf";
import { authorizationHeaderValue } from "../services/authorization";

export const useFeedbackStore = defineStore("feedback", () => {
  const recorder = new DiagnosticRecorder();
  const formRevision = ref(0);
  const open = ref(false),
    enabled = ref(false),
    storageError = ref(null),
    selectedTurn = ref(null);
  const preview = shallowRef(null),
    receipt = ref(null),
    error = ref(null),
    busy = ref(false);
  const reports = ref([]),
    cursor = ref(null);
  let owner = null,
    generation = 0,
    controller = null;
  function invalidate() {
    formRevision.value++;
    selectedTurn.value = null;
    generation++;
    controller?.abort();
    controller = null;
    preview.value = null;
    receipt.value = null;
    error.value = null;
    reports.value = [];
    cursor.value = null;
  }
  async function account(handle) {
    invalidate();
    open.value = false;
    owner = handle ? JSON.stringify([window.location.origin, handle]) : null;
    await recorder.account(owner);
    sync();
  }
  function sync() {
    enabled.value = recorder.enabled;
    storageError.value = recorder.error;
  }
  function broadcastClear() {
    try {
      localStorage.setItem("diagnostic_clear", crypto.randomUUID());
    } catch {
      /* best effort cross-tab notification */
    }
  }
  window.addEventListener?.("storage", (event) => {
    if (event.key === "diagnostic_clear" || event.key === "ai_chat_owner") {
      invalidate();
      open.value = false;
      recorder.forget(event.key === "ai_chat_owner");
      if (event.key === "ai_chat_owner") owner = null;
      sync();
    }
  });
  async function setEnabled(value) {
    if (!owner) {
      error.value = "Sign in before enabling diagnostics.";
      return;
    }
    broadcastClear();
    invalidate();
    await recorder.enable(value);
    sync();
  }
  async function clear() {
    broadcastClear();
    invalidate();
    await recorder.clear();
    sync();
  }
  function record(
    turn,
    conversation,
    event,
    status,
    expected = recorder.generation,
  ) {
    if (expected !== recorder.generation) return;
    void Promise.resolve(
      recorder.event(turn, conversation, event, status),
    ).then(sync);
    sync();
  }
  function show(turn = null) {
    formRevision.value++;
    receipt.value = null;
    preview.value = null;
    selectedTurn.value = turn;
    open.value = true;
    error.value = null;
  }
  async function request(path, options = {}, expected = generation) {
    if (!owner || expected !== generation)
      throw new Error("Account or diagnostics changed; review a new report.");
    const capturedOwner = owner;
    const token =
      (await oidc.getIdToken()) || localStorage.getItem("auth_token");
    if (expected !== generation || owner !== capturedOwner)
      throw new Error("Account changed; review a new report.");
    const headers = { "Content-Type": "application/json" };
    const csrf = getCsrfToken();
    if (csrf) headers["X-CSRF-Token"] = csrf;
    if (!token)
      throw new Error(
        "Sign in again to submit reports with account-bound credentials.",
      );
    headers.Authorization = authorizationHeaderValue(token);
    const identity = await fetch(
      new URL("/v1/auth/session", window.location.origin),
      {
        headers: { Authorization: headers.Authorization },
        credentials: "omit",
        cache: "no-store",
        redirect: "error",
      },
    );
    if (
      !identity.ok ||
      (await identity.json()).user_handle !== JSON.parse(capturedOwner)[1]
    )
      throw new Error(
        "Account changed or session expired. Sign in and review a new report.",
      );
    if (expected !== generation || owner !== capturedOwner)
      throw new Error("Account changed; review a new report.");
    controller = new AbortController();
    const response = await fetch(
      new URL(`/v1/reports${path}`, window.location.origin),
      {
        ...options,
        headers,
        credentials: "omit",
        cache: "no-store",
        redirect: "error",
        signal: controller.signal,
      },
    );
    if (expected !== generation)
      throw new Error("Account or diagnostics changed; review a new report.");
    if (!response.ok) {
      const messages = {
        401: "Session expired. Sign in and review a new report.",
        403: "Report permission is required.",
        409: "This request ID conflicts with another report. Edit and review again.",
        413: "The report exceeds the server size limit. Reduce the selected content.",
        429: "Report quota reached.",
        503: "Report storage is at capacity.",
      };
      const retry = response.headers.get("Retry-After");
      throw new Error(
        `${messages[response.status] || `Report request failed (${response.status}).`}${retry ? ` Retry after ${retry} seconds.` : ""}`,
      );
    }
    return response.status === 204 ? null : response.json();
  }
  async function freeze(form) {
    error.value = null;
    try {
      if (!owner) throw new Error("Sign in before reporting.");
      if (
        !form.description.trim() ||
        bytes(form.description) > 102400 ||
        bytes(form.title) > 200
      )
        throw new Error(
          "Enter a description up to 100 KiB and a title up to 200 UTF-8 bytes.",
        );
      const attachments = [];
      if (form.conversation)
        attachments.push({
          kind: "assistant",
          content: recorder.snapshot(selectedTurn.value, form.allTurns),
          consent: true,
        });
      if (form.logs) {
        const content = redact(form.technicalLogs);
        if (bytes(content) > 1048576)
          throw new Error("Technical logs exceed 1 MiB.");
        attachments.push({ kind: "technical_logs", content, consent: true });
      }
      const payload = {
        client_request_id: crypto.randomUUID(),
        kind: form.kind,
        category: form.category,
        title: form.title || null,
        description: form.description,
        client_type: "web",
        attachments,
      };
      preview.value = Object.freeze({
        body: JSON.stringify(payload),
        generation,
        owner,
      });
    } catch (e) {
      error.value = e.message;
    }
  }
  async function send() {
    if (busy.value || !preview.value) return;
    const snapshot = preview.value;
    busy.value = true;
    error.value = null;
    try {
      if (snapshot.owner !== owner || snapshot.generation !== generation)
        throw new Error("Account or diagnostics changed; review a new report.");
      const result = await request(
        "",
        { method: "POST", body: snapshot.body },
        snapshot.generation,
      );
      if (snapshot.generation === generation) {
        receipt.value = result.id;
        preview.value = null;
      }
    } catch (e) {
      if (snapshot.generation === generation) error.value = e.message;
    } finally {
      busy.value = false;
    }
  }
  async function list(more = false) {
    const expected = generation;
    error.value = null;
    try {
      const page = await request(
        more && cursor.value ? `?before=${cursor.value}` : "",
        {},
        expected,
      );
      if (expected !== generation) return;
      reports.value = more ? [...reports.value, ...page.items] : page.items;
      cursor.value = page.next_cursor;
    } catch (e) {
      if (expected === generation) error.value = e.message;
    }
  }
  return {
    epoch: () => recorder.generation,
    formRevision,
    open,
    enabled,
    storageError,
    selectedTurn,
    preview,
    receipt,
    error,
    busy,
    reports,
    cursor,
    account,
    setEnabled,
    clear,
    record,
    show,
    freeze,
    send,
    list,
    edit: () => {
      preview.value = null;
    },
  };
});
