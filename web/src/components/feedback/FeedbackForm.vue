<script setup>
import { reactive, watch } from "vue";
import { useFeedbackStore } from "../../store/feedback";
const feedback = useFeedbackStore();
const defaults = () => ({
  kind: "bug",
  category: "assistant",
  title: "",
  description: "",
  conversation: false,
  allTurns: false,
  logs: false,
  technicalLogs: "",
});
const form = reactive(defaults());
watch(
  () => feedback.formRevision,
  () => Object.assign(form, defaults()),
);
</script>
<template>
  <div v-if="feedback.open" class="feedback-overlay">
    <section
      role="dialog"
      aria-modal="true"
      aria-label="Feedback report"
      class="feedback-dialog"
    >
      <button @click="feedback.open = false">Close</button>
      <h2>Feedback report</h2>
      <p>
        Attachments can contain personal data. Review the full preview before
        sending. Local clear does not delete submitted reports.
      </p>
      <p v-if="feedback.error" role="alert">{{ feedback.error }}</p>
      <p v-if="feedback.receipt" role="status">
        Submitted report: {{ feedback.receipt }}
      </p>
      <template v-if="feedback.preview">
        <h3>Fixed report preview</h3>
        <pre>{{ feedback.preview.body }}</pre>
        <button :disabled="feedback.busy" @click="feedback.send()">
          Send / retry this exact report
        </button>
        <button :disabled="feedback.busy" @click="feedback.edit()">
          Edit (creates a new request)
        </button>
      </template>
      <form v-else @submit.prevent="feedback.freeze(form)">
        <label
          >Kind
          <select v-model="form.kind">
            <option value="bug">Bug</option>
            <option value="feature">Feature request</option>
          </select></label
        >
        <label
          >Category
          <select v-model="form.category">
            <option
              v-for="category in [
                'assistant',
                'playback',
                'downloads',
                'ui',
                'other',
              ]"
              :key="category"
              :value="category"
            >
              {{ category }}
            </option>
          </select></label
        >
        <label>Title <input v-model="form.title" /></label>
        <label
          >Description <textarea v-model="form.description" required />
        </label>
        <label
          ><input
            v-model="form.conversation"
            type="checkbox"
            :disabled="!feedback.enabled"
          />
          Attach selected response diagnostics (off by default)</label
        >
        <label v-if="form.conversation"
          ><input v-model="form.allTurns" type="checkbox" /> Include all
          retained turns instead of the selected turn</label
        >
        <p v-if="form.conversation && !feedback.selectedTurn && !form.allTurns">
          Select a response from the assistant reporting controls, or explicitly
          include all retained turns.
        </p>
        <label
          ><input v-model="form.logs" type="checkbox" /> Attach technical logs
          (off by default)</label
        >
        <label v-if="form.logs"
          >Paste technical logs to preview
          <textarea v-model="form.technicalLogs" />
        </label>
        <button>Review before sending</button>
      </form>
      <h3>My reports</h3>
      <button @click="feedback.list()">Refresh status</button>
      <ul>
        <li v-for="report in feedback.reports" :key="report.id">
          {{ report.id }} — {{ report.title || report.kind }} —
          {{ report.status }}
        </li>
      </ul>
      <button v-if="feedback.cursor" @click="feedback.list(true)">
        More reports
      </button>
    </section>
  </div>
</template>
<style scoped>
.feedback-overlay {
  position: fixed;
  inset: 0;
  z-index: 10000;
  background: #000b;
  display: grid;
  place-items: center;
}
.feedback-dialog {
  background: var(--bg-primary, #181818);
  color: var(--text-primary, white);
  padding: 24px;
  width: min(680px, 94vw);
  max-height: 90vh;
  overflow: auto;
}
label {
  display: block;
  margin: 12px 0;
}
textarea {
  display: block;
  width: 100%;
  min-height: 80px;
}
pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 40vh;
  overflow: auto;
}
button {
  margin: 6px;
}
</style>
