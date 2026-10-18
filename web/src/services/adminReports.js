import axios from "axios";

export const statuses = [
  "new",
  "investigating",
  "planned",
  "resolved",
  "closed",
];
export const categories = ["assistant", "playback", "downloads", "ui", "other"];
const base = "/v1/admin/reports";
const id = (value) => encodeURIComponent(value);
export function createAdminReportsApi(http = axios) {
  const request = async (method, path = "", data, params) =>
    (await http.request({ method, url: base + path, data, params })).data;
  return {
    list: (params) => request("get", "", undefined, params),
    detail: (key) => request("get", `/${id(key)}`),
    update: (key, data) => request("patch", `/${id(key)}`, data),
    notes: (key, before) =>
      request("get", `/${id(key)}/notes`, undefined, { before, limit: 25 }),
    events: (key, before) =>
      request("get", `/${id(key)}/events`, undefined, { before, limit: 25 }),
    attachment: (key, attachment) =>
      request("get", `/${id(key)}/attachments/${id(attachment)}`),
    deleteAttachment: (key, attachment) =>
      request("delete", `/${id(key)}/attachments/${id(attachment)}`),
    stats: () => request("get", "/stats"),
    settings: () => request("get", "/settings"),
    saveSettings: (data) => request("put", "/settings", data),
    integrations: () => request("get", "/integrations"),
    toggleIntegration: (item) =>
      request("patch", `/integrations/${id(item.id)}`, {
        expected_version: item.version,
        enabled: !item.enabled,
      }),
    deliveries: (params) => request("get", "/deliveries", undefined, params),
    retry: (item) =>
      request("post", `/deliveries/${id(item.id)}/retry`, {
        expected_version: item.version,
      }),
  };
}
export function reportError(error) {
  const status = error.response?.status;
  const retry = error.response?.headers?.["retry-after"];
  const messages = {
    400: "Invalid input. Check the fields and limits. Duplicate IDs must exist and cannot form a cycle.",
    401: "Your session expired. Sign in again.",
    403: "Permission denied. Ask an administrator for the required report permission.",
    404: "This resource is unavailable. Refresh its metadata.",
    409: "This resource changed or the action is no longer available. Refresh, review the latest version, then try again.",
    413: "The request is too large. Shorten the note or reduce the submitted data.",
    429: "Rate limit reached. Wait before trying again.",
    503: "Server capacity is full. Free diagnostic storage or wait before trying again.",
  };
  return (
    (messages[status] ||
      "The operation failed. Check your connection and retry manually.") +
    (retry ? ` Retry-After: ${retry}.` : "")
  );
}
export function transcript(content) {
  try {
    const value = JSON.parse(content);
    if (value.schema_version !== 1 || !Array.isArray(value.turns)) return null;
    return value;
  } catch {
    return null;
  }
}
export function filters(values) {
  return Object.fromEntries(
    Object.entries(values).filter(
      ([, value]) => value !== "" && value !== undefined && value !== null,
    ),
  );
}
