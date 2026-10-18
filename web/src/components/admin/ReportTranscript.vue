<template>
  <section aria-label="Attachment content">
    <button type="button" @click="$emit('close')">
      Close sensitive content
    </button>
    <template v-if="parsed">
      <p>
        Diagnostic schema {{ parsed.schema_version }} · Captured
        {{ parsed.captured_at }}
      </p>
      <details v-for="(turn, index) in parsed.turns" :key="index">
        <summary>
          Turn {{ turn.id }} · {{ turn.status }} · {{ turn.started_at }}
        </summary>
        <p>Conversation: {{ turn.conversation_id }}</p>
        <article v-for="(event, eventIndex) in turn.events" :key="eventIndex">
          <h5>
            {{ event.kind }} · {{ event.timestamp }} · {{ event.tool_name }}
          </h5>
          <pre>{{ event.content }}</pre>
          <pre v-if="event.data">{{ JSON.stringify(event.data, null, 2) }}</pre>
        </article>
      </details>
    </template>
    <template v-else>
      <p v-if="kind === 'assistant'">
        Unrecognized transcript schema. Showing safe plain text.
      </p>
      <pre>{{ content }}</pre>
    </template>
  </section>
</template>
<script setup>
import { computed } from "vue";
import { transcript } from "@/services/adminReports";
const props = defineProps({
  content: { type: String, required: true },
  kind: { type: String, required: true },
});
defineEmits(["close"]);
const parsed = computed(() =>
  props.kind === "assistant" ? transcript(props.content) : null,
);
</script>
<style scoped>
:focus-visible {
  outline: 2px solid #72d8ff;
  outline-offset: 3px;
}
pre {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 24rem;
  overflow: auto;
}
article {
  border-left: 2px solid var(--border-subdued);
  padding-left: 1rem;
}
summary {
  cursor: pointer;
}
</style>
