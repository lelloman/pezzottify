<template>
  <section
    class="reports"
    aria-label="Feedback administration"
    :aria-busy="busy"
  >
    <h2>Feedback Reports</h2>
    <p v-if="busy">Loading or saving…</p>
    <p v-if="error" role="alert">{{ error }}</p>
    <p v-if="message" role="status">{{ message }}</p>
    <div v-if="canTriage">
      <h3>Inbox</h3>
      <form @submit.prevent="loadReports()">
        <label
          >Kind
          <select :disabled="busy" v-model="query.kind" aria-label="Kind">
            <option value="">All</option>
            <option>bug</option>
            <option>feature</option>
          </select></label
        >
        <label
          >Category
          <select
            :disabled="busy"
            v-model="query.category"
            aria-label="Category"
          >
            <option value="">All</option>
            <option v-for="item in categories" :key="item">{{ item }}</option>
          </select></label
        >
        <label
          >Status
          <select :disabled="busy" v-model="query.status" aria-label="Status">
            <option value="">All</option>
            <option v-for="item in statuses" :key="item">{{ item }}</option>
          </select></label
        >
        <label
          >Client
          <select
            :disabled="busy"
            v-model="query.client_type"
            aria-label="Client"
          >
            <option value="">All</option>
            <option>web</option>
            <option>android</option>
          </select></label
        >
        <label
          >Client version
          <input :disabled="busy" v-model="query.client_version"
        /></label>
        <label
          >User ID
          <input :disabled="busy" v-model="query.user_id" type="number" min="0"
        /></label>
        <label
          >Diagnostics
          <select
            :disabled="busy"
            v-model="query.has_diagnostics"
            aria-label="Diagnostics"
          >
            <option value="">All</option>
            <option value="true">Present</option>
            <option value="false">Absent</option>
          </select></label
        >
        <button :disabled="busy">Apply filters / Refresh inbox</button>
      </form>
      <p v-if="page && !page.items.length">No reports match these filters.</p>
      <ul v-if="page">
        <li v-for="item in page.items" :key="item.id">
          <button :disabled="busy" @click="openReport(item.id)">
            {{ item.title || "(No title)" }} · {{ item.id }}
          </button>
          {{ item.kind }} / {{ item.category }} / {{ item.status }} ·
          {{ item.user_handle }} · {{ item.client_type }}
          {{ item.client_version }}
        </li>
      </ul>
      <button
        v-if="page?.next_cursor"
        :disabled="busy"
        @click="loadReports(page.next_cursor)"
      >
        Next inbox page
      </button>
      <button :disabled="busy" @click="loadStats">
        Refresh storage and rejection statistics
      </button>
      <section v-if="stats" aria-label="Storage and rejection statistics">
        <h3>Storage and rejection statistics</h3>
        <p>
          Reports: {{ stats.storage.reports }} /
          {{ stats.storage.settings.total_reports }} · Diagnostic bytes:
          {{ stats.storage.diagnostic_bytes }} /
          {{ stats.storage.settings.diagnostic_bytes }} · Metadata bytes:
          {{ stats.storage.metadata_bytes }} /
          {{ stats.storage.settings.metadata_bytes }}
        </p>
        <p>
          Retention: {{ stats.storage.settings.retention_days }} days.
          Expiration preserves reports; backups follow a separate retention
          policy.
        </p>
        <pre>{{
          JSON.stringify(
            { admission: stats.admission, deliveries: stats.deliveries },
            null,
            2,
          )
        }}</pre>
      </section>
    </div>

    <section v-if="canTriage && report" aria-label="Selected report">
      <h3>{{ report.title || "(No title)" }} · {{ report.id }}</h3>
      <p>
        Version {{ report.version }} · {{ report.status }} ·
        {{ report.created_at }}
      </p>
      <pre>{{ report.description }}</pre>
      <p>
        Client: {{ report.client_type }} {{ report.client_version }} ·
        {{ report.device_info }}
      </p>
      <p>Duplicate of: {{ report.duplicate_of || "None" }}</p>
      <button :disabled="busy" @click="openReport(report.id)">
        Refresh selected report
      </button>
      <form @submit.prevent="saveReport">
        <label
          >New status
          <select
            :disabled="busy"
            v-model="edit.status"
            aria-label="New status"
          >
            <option v-for="item in statuses" :key="item">{{ item }}</option>
          </select></label
        >
        <label
          >Duplicate report ID (empty clears link)
          <input :disabled="busy" v-model="edit.duplicate_of" maxlength="128"
        /></label>
        <label
          >Internal note <textarea :disabled="busy" v-model="edit.note" />
        </label>
        <button :disabled="busy">Save triage</button>
      </form>
      <h4>Internal notes</h4>
      <button :disabled="busy" @click="loadNotes()">Refresh notes</button>
      <ul v-if="notes">
        <li v-for="item in notes.items" :key="item.id">
          {{ time(item.created_at) }} · Actor {{ item.actor_id }}
          <pre>{{ item.data.text }}</pre>
        </li>
      </ul>
      <button
        v-if="notes?.next_cursor"
        :disabled="busy"
        @click="loadNotes(notes.next_cursor)"
      >
        Next notes page
      </button>
      <h4>Attachments</h4>
      <p>
        Deleting an attachment preserves report metadata. Expiration cannot be
        reversed.
      </p>
      <ul>
        <li v-for="item in report.attachments" :key="item.id">
          {{ item.kind }} · {{ item.size_bytes }} UTF-8 bytes ·
          {{ item.state }} · Created {{ time(item.created_at) }} · Expires
          {{ time(item.expires_at) }}
          <template v-if="canDiagnose">
            <button
              :disabled="busy || item.state !== 'active'"
              @click="readAttachment(report.id, item.id)"
            >
              View {{ item.kind }}
            </button>
            <button
              :disabled="busy || item.state !== 'active'"
              @click="deletion = { report: report.id, attachment: item.id }"
            >
              Delete {{ item.kind }} attachment
            </button>
          </template>
        </li>
      </ul>
    </section>

    <section v-if="canDiagnose" aria-label="Diagnostics and audit">
      <h3>Diagnostics and audit</h3>
      <p>
        Access is audited. Uploaded text and tool calls are displayed as text
        only.
      </p>
      <form
        @submit.prevent="readAttachment(diagnosticReport, diagnosticAttachment)"
      >
        <label
          >Report ID
          <input :disabled="busy" v-model="diagnosticReport" required
        /></label>
        <label
          >Attachment ID
          <input :disabled="busy" v-model="diagnosticAttachment" required
        /></label>
        <button :disabled="busy">View attachment by ID</button>
        <button
          type="button"
          :disabled="busy || !diagnosticReport || !diagnosticAttachment"
          @click="
            deletion = {
              report: diagnosticReport,
              attachment: diagnosticAttachment,
            }
          "
        >
          Delete attachment by ID
        </button>
      </form>
      <ReportTranscript
        v-if="attachment"
        :content="attachment.content"
        :kind="attachment.attachment.kind"
        @close="attachment = null"
      />
      <form @submit.prevent="loadEvents()">
        <label
          >Audit resource (report ID, settings, administration, automation)
          <input :disabled="busy" v-model="auditResource" required
        /></label>
        <button :disabled="busy">Load audit trail</button>
      </form>
      <h4 v-if="events">Audit trail for {{ appliedAuditResource }}</h4>
      <ul v-if="events">
        <li v-for="item in events.items" :key="item.id">
          {{ time(item.created_at) }} · Actor {{ item.actor_id }} ·
          {{ item.kind }} · {{ item.id }}
          <pre>{{ JSON.stringify(item.data, null, 2) }}</pre>
        </li>
      </ul>
      <button
        v-if="events?.next_cursor"
        :disabled="busy"
        @click="loadEvents(events.next_cursor)"
      >
        Next audit page
      </button>
    </section>

    <section v-if="canIntegrate" aria-label="Automation controls">
      <h3>Automation controls</h3>
      <p>
        Only metadata is forwarded. Disabling prevents future claims; an
        in-flight delivery cannot be recalled. Destinations are configured
        separately by an administrator.
      </p>
      <button :disabled="busy" @click="loadIntegrations">
        Refresh integrations
      </button>
      <ul>
        <li v-for="item in integrations" :key="item.id">
          {{ item.id }} · {{ item.url }} ·
          {{ item.enabled ? "Enabled" : "Disabled" }} · Version
          {{ item.version }}
          <button :disabled="busy" @click="integrationChange = item">
            {{ item.enabled ? "Disable" : "Enable" }} integration
          </button>
        </li>
      </ul>
      <form @submit.prevent="loadDeliveries()">
        <label
          >Delivery state
          <select
            :disabled="busy"
            v-model="deliveryQuery.state"
            aria-label="Delivery state"
          >
            <option value="">All</option>
            <option>pending</option>
            <option>leased</option>
            <option>succeeded</option>
            <option>failed</option>
          </select></label
        >
        <label
          >Integration ID
          <input :disabled="busy" v-model="deliveryQuery.integration_id"
        /></label>
        <button :disabled="busy">Refresh deliveries</button>
      </form>
      <ul v-if="deliveries">
        <li v-for="item in deliveries.items" :key="item.id">
          {{ item.id }} · Event {{ item.event_id }} · Integration
          {{ item.integration_id }} · {{ item.state }} · {{ item.outcome }} ·
          Attempts {{ item.attempts }} · Manual retries
          {{ item.manual_retries }} / 5 · Next {{ time(item.next_at) }} ·
          Version {{ item.version }}
          <button
            :disabled="
              busy || item.state !== 'failed' || item.manual_retries >= 5
            "
            @click="retryDelivery = item"
          >
            Retry delivery
          </button>
        </li>
      </ul>
      <button
        v-if="deliveries?.next_cursor"
        :disabled="busy"
        @click="loadDeliveries(deliveries.next_cursor)"
      >
        Next delivery page
      </button>
      <h4>Limits and attachment retention</h4>
      <button :disabled="busy" @click="loadSettings">Refresh settings</button>
      <form v-if="settings" @submit.prevent="settingsChange = true">
        <p>
          Version {{ settings.version }}. Shortening retention also shortens
          existing deadlines; increasing it never restores or extends existing
          attachments.
        </p>
        <label v-for="key in settingKeys" :key="key"
          >{{ key.replaceAll("_", " ") }}
          <input
            v-model.number="settings[key]"
            type="number"
            min="1"
            :max="settingLimits[key]"
            required
        /></label>
        <button :disabled="busy">Save limits and retention</button>
      </form>
    </section>

    <ConfirmationDialog
      :isOpen="!!deletion"
      :closeCallback="() => (deletion = null)"
      :positiveButtonCallback="deleteAttachment"
      title="Delete attachment"
      positiveButtonText="Delete attachment"
      negativeButtonText="Cancel"
    >
      <template #message
        >Delete attachment {{ deletion?.attachment }} from report
        {{ deletion?.report }}? The report remains. This cannot be
        undone.</template
      >
    </ConfirmationDialog>
    <ConfirmationDialog
      :isOpen="!!integrationChange"
      :closeCallback="() => (integrationChange = null)"
      :positiveButtonCallback="toggleIntegration"
      title="Change integration"
      positiveButtonText="Confirm"
      negativeButtonText="Cancel"
    >
      <template #message
        >{{
          integrationChange?.enabled
            ? "Disable future delivery claims to"
            : "Enable metadata delivery to"
        }}
        {{ integrationChange?.url }}?</template
      >
    </ConfirmationDialog>
    <ConfirmationDialog
      :isOpen="!!retryDelivery"
      :closeCallback="() => (retryDelivery = null)"
      :positiveButtonCallback="retry"
      title="Retry delivery"
      positiveButtonText="Retry once"
      negativeButtonText="Cancel"
    >
      <template #message
        >Grant one additional attempt for {{ retryDelivery?.id }}? Receivers may
        already have received this event.</template
      >
    </ConfirmationDialog>
    <ConfirmationDialog
      :isOpen="settingsChange"
      :closeCallback="() => (settingsChange = false)"
      :positiveButtonCallback="saveSettings"
      title="Update limits and retention"
      positiveButtonText="Save"
      negativeButtonText="Cancel"
    >
      <template #message
        >Apply these limits? Shorter retention may permanently expire existing
        attachments.</template
      >
    </ConfirmationDialog>
  </section>
</template>
<script setup>
import {
  ref,
  reactive,
  computed,
  watch,
  onMounted,
  onBeforeUnmount,
} from "vue";
import { useAuthStore } from "@/store/auth";
import { useUserStore } from "@/store/user";
import {
  createAdminReportsApi,
  reportError,
  filters,
  statuses,
  categories,
} from "@/services/adminReports";
import ConfirmationDialog from "./ReportConfirmation.vue";
import ReportTranscript from "./ReportTranscript.vue";
const user = useUserStore();
const auth = useAuthStore();
const api = createAdminReportsApi();
const canTriage = computed(() => user.hasPermission("TriageReports"));
const canDiagnose = computed(() => user.hasPermission("ViewReportDiagnostics"));
const canIntegrate = computed(() =>
  user.hasPermission("ManageReportIntegrations"),
);
const busy = ref(false),
  error = ref(""),
  message = ref("");
const page = ref(null),
  report = ref(null),
  notes = ref(null),
  events = ref(null),
  stats = ref(null);
const attachment = ref(null),
  settings = ref(null),
  integrations = ref([]),
  deliveries = ref(null);
const deletion = ref(null),
  integrationChange = ref(null),
  retryDelivery = ref(null),
  settingsChange = ref(false);
const diagnosticReport = ref(""),
  diagnosticAttachment = ref(""),
  auditResource = ref("automation");
const query = reactive({
  kind: "",
  category: "",
  status: "",
  client_type: "",
  client_version: "",
  user_id: "",
  has_diagnostics: "",
});
const deliveryQuery = reactive({ state: "", integration_id: "" });
const edit = reactive({ status: "new", duplicate_of: "", note: "" });
const settingKeys = [
  "hourly_reports",
  "daily_reports",
  "daily_attachment_bytes",
  "user_reports",
  "total_reports",
  "metadata_bytes",
  "diagnostic_bytes",
  "retention_days",
];
const settingLimits = {
  hourly_reports: 5,
  daily_reports: 20,
  daily_attachment_bytes: 33554432,
  user_reports: 1000,
  total_reports: 100000,
  metadata_bytes: 268435456,
  diagnostic_bytes: 524288000,
  retention_days: 90,
};
let generation = 0;
let inboxFilters = {},
  appliedDeliveryFilters = {},
  appliedAuditResource = "";
// One explicit operation at a time. Invalidated requests cannot restore sensitive data.
async function run(operation, permission) {
  if (busy.value || !permission.value) return;
  const token = generation;
  busy.value = true;
  error.value = "";
  message.value = "";
  const publish = (fn) => {
    if (token === generation && permission.value) fn();
  };
  try {
    await operation(publish);
  } catch (e) {
    publish(() => {
      error.value = reportError(e);
    });
  } finally {
    if (token === generation) busy.value = false;
  }
}
function setReport(value) {
  report.value = value;
  Object.assign(edit, {
    status: value.status,
    duplicate_of: value.duplicate_of || "",
    note: "",
  });
}
function loadReports(before) {
  if (!before) inboxFilters = filters(query);
  return run(async (publish) => {
    const value = await api.list({ ...inboxFilters, before, limit: 25 });
    publish(() => {
      page.value = value;
    });
  }, canTriage);
}
function openReport(key) {
  attachment.value = null;
  notes.value = null;
  return run(async (publish) => {
    publish(() => {
      report.value = null;
    });
    const value = await api.detail(key);
    publish(() => {
      setReport(value);
      diagnosticReport.value = value.id;
      auditResource.value = value.id;
      events.value = null;
    });
  }, canTriage);
}
function saveReport() {
  const key = report.value.id;
  const body = {
    expected_version: report.value.version,
    status: edit.status,
    duplicate_of: edit.duplicate_of,
  };
  if (edit.note.trim()) body.note = edit.note;
  return run(async (publish) => {
    const value = await api.update(key, body);
    publish(() => {
      setReport(value);
      notes.value = null;
      message.value =
        "Triage saved. Refresh notes to view the updated history.";
    });
  }, canTriage);
}
function loadNotes(before) {
  return run(async (publish) => {
    const value = await api.notes(report.value.id, before);
    publish(() => {
      notes.value = value;
    });
  }, canTriage);
}
function loadStats() {
  return run(async (publish) => {
    const value = await api.stats();
    publish(() => {
      stats.value = value;
    });
  }, canTriage);
}
function loadEvents(before) {
  if (!before) appliedAuditResource = auditResource.value;
  return run(async (publish) => {
    const value = await api.events(appliedAuditResource, before);
    publish(() => {
      events.value = value;
    });
  }, canDiagnose);
}
function readAttachment(key, item) {
  attachment.value = null;
  return run(async (publish) => {
    const value = await api.attachment(key, item);
    publish(() => {
      attachment.value = value;
    });
  }, canDiagnose);
}
function deleteAttachment() {
  const item = deletion.value;
  deletion.value = null;
  return run(async (publish) => {
    await api.deleteAttachment(item.report, item.attachment);
    publish(() => {
      attachment.value = null;
      report.value = null;
      message.value =
        "Attachment deleted. Refresh report metadata and statistics.";
    });
  }, canDiagnose);
}
function loadIntegrations() {
  return run(async (publish) => {
    const value = await api.integrations();
    publish(() => {
      integrations.value = value;
    });
  }, canIntegrate);
}
function toggleIntegration() {
  const item = integrationChange.value;
  integrationChange.value = null;
  return run(async (publish) => {
    await api.toggleIntegration(item);
    publish(() => {
      integrations.value = [];
      message.value =
        "Integration updated. Refresh integrations to see the current state.";
    });
  }, canIntegrate);
}
function loadDeliveries(before) {
  if (!before) appliedDeliveryFilters = filters(deliveryQuery);
  return run(async (publish) => {
    const value = await api.deliveries({
      ...appliedDeliveryFilters,
      before,
      limit: 25,
    });
    publish(() => {
      deliveries.value = value;
    });
  }, canIntegrate);
}
function retry() {
  const item = retryDelivery.value;
  retryDelivery.value = null;
  return run(async (publish) => {
    await api.retry(item);
    publish(() => {
      deliveries.value = null;
      message.value =
        "Retry scheduled. Refresh deliveries for its current state.";
    });
  }, canIntegrate);
}
function loadSettings() {
  return run(async (publish) => {
    const value = await api.settings();
    publish(() => {
      settings.value = value;
    });
  }, canIntegrate);
}
function saveSettings() {
  settingsChange.value = false;
  const body = { ...settings.value };
  return run(async (publish) => {
    const value = await api.saveSettings(body);
    publish(() => {
      settings.value = value;
      message.value = "Limits and retention updated.";
    });
  }, canIntegrate);
}
function time(value) {
  return new Date(value * 1000).toLocaleString();
}
function clear() {
  generation++;
  busy.value = false;
  page.value =
    report.value =
    notes.value =
    events.value =
    stats.value =
    attachment.value =
    settings.value =
    deliveries.value =
      null;
  integrations.value = [];
  deletion.value = integrationChange.value = retryDelivery.value = null;
  settingsChange.value = false;
  edit.note = "";
  error.value = message.value = "";
  diagnosticReport.value = diagnosticAttachment.value = "";
  auditResource.value = "automation";
  Object.assign(edit, { status: "new", duplicate_of: "", note: "" });
  for (const key of Object.keys(query)) query[key] = "";
  Object.assign(deliveryQuery, { state: "", integration_id: "" });
  inboxFilters = {};
  appliedDeliveryFilters = {};
  appliedAuditResource = "";
}
watch(
  () => [
    auth.user?.handle,
    canTriage.value,
    canDiagnose.value,
    canIntegrate.value,
  ],
  clear,
  { flush: "sync" },
);
onBeforeUnmount(clear);
onMounted(() => {
  if (canTriage.value) loadReports();
  else if (canIntegrate.value) loadIntegrations();
});
</script>
<style scoped>
:focus-visible {
  outline: 2px solid #72d8ff;
  outline-offset: 3px;
}
.reports {
  max-width: 1100px;
}
section {
  margin-block: 1rem;
}
form {
  display: flex;
  flex-wrap: wrap;
  align-items: end;
  gap: 0.8rem;
  margin-block: 1rem;
}
label {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
input,
select,
textarea,
button {
  font: inherit;
  padding: 0.4rem;
  color: var(--text-base);
  background: var(--bg-elevated-base);
  border: 1px solid var(--border-subdued);
  border-radius: 0.3rem;
}
button {
  cursor: pointer;
}
button:disabled {
  opacity: 0.5;
  cursor: default;
}
li {
  margin-block: 0.7rem;
  overflow-wrap: anywhere;
}
pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 24rem;
  overflow: auto;
}
[role="alert"] {
  color: #e66;
}
</style>
