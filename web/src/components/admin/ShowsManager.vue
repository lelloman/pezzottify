<template>
  <div class="showsManager">
    <section class="draftPanel">
      <div>
        <h2>Shows</h2>
        <p>Create a draft, edit the script, synthesize narration, then publish.</p>
      </div>
      <form class="draftForm" @submit.prevent="createDraft">
        <textarea v-model="draftBrief" placeholder="Show brief, audience, mood, topics, repertoire notes" rows="3"></textarea>
        <div class="draftControls">
          <label>
            <span>Duration</span>
            <input v-model.number="targetMinutes" type="number" min="10" max="120" />
          </label>
          <label>
            <span>Language</span>
            <select v-model="language">
              <option v-for="option in languageOptions" :key="option.code" :value="option.code">
                {{ option.label }}
              </option>
            </select>
          </label>
          <button type="submit" class="primaryButton" :disabled="isBusy || !draftBrief.trim()">Create Draft</button>
        </div>
      </form>
    </section>

    <div class="layout">
      <aside class="showList">
        <button
          v-for="show in shows"
          :key="show.id"
          class="showListItem"
          :class="{ active: selectedShow?.id === show.id }"
          @click="selectShow(show.id)"
        >
          <span>{{ show.title }}</span>
          <small>{{ show.status }} · {{ show.track_count }} tracks</small>
        </button>
        <div v-if="shows.length === 0" class="emptyState">No show drafts yet.</div>
      </aside>

      <section v-if="selectedShow" class="editorPanel">
        <div class="editorHeader">
          <div>
            <input v-model="selectedShow.title" class="titleInput" />
            <textarea v-model="selectedShow.summary" class="summaryInput" rows="2" />
          </div>
          <div class="actions">
            <button class="secondaryButton" @click="saveScript" :disabled="isBusy">Save</button>
            <button class="secondaryButton" @click="synthesize" :disabled="isBusy">Synthesize</button>
            <button class="primaryButton" @click="publish" :disabled="isBusy || selectedShow.status !== 'ready'">Publish</button>
            <button class="dangerButton" @click="deleteCurrent" :disabled="isBusy">Delete</button>
          </div>
        </div>

        <div class="metaRow">
          <label>
            Language
            <select v-model="selectedShow.language">
              <option v-for="option in languageOptions" :key="option.code" :value="option.code">
                {{ option.label }}
              </option>
            </select>
          </label>
          <label>Target min <input v-model.number="selectedShow.target_duration_minutes" type="number" min="1" max="180" /></label>
          <span class="status">{{ selectedShow.status }}</span>
        </div>

        <div v-if="selectedShow.error" class="errorBox">{{ selectedShow.error }}</div>

        <section class="speakers">
          <h3>Cast</h3>
          <div v-for="speaker in selectedShow.speakers" :key="speaker.id" class="speakerRow">
            <input v-model="speaker.name" />
            <input v-model="speaker.voice_id" placeholder="voice id" />
          </div>
        </section>

        <section class="segments">
          <h3>Timeline</h3>
          <article v-for="segment in selectedShow.segments" :key="segment.id" class="segmentEditor">
            <div class="segmentHeader">
              <span>{{ segment.kind }}</span>
              <input v-model="segment.title" />
            </div>
            <textarea
              v-if="segment.kind === 'narration'"
              v-model="segment.text"
              rows="4"
              placeholder="Narration text"
            />
            <div v-else class="trackRef">Track: {{ segment.track_id }}</div>
          </article>
        </section>
      </section>

      <section v-else class="emptyEditor">Select a show or create a new draft.</section>
    </div>
  </div>
</template>

<script setup>
import { onMounted, ref } from "vue";
import { useRemoteStore } from "@/store/remote";

const remote = useRemoteStore();
const shows = ref([]);
const selectedShow = ref(null);
const isBusy = ref(false);
const draftBrief = ref("");
const targetMinutes = ref(75);
const language = ref("en");
const languageOptions = [
  { code: "en", label: "English" },
  { code: "it", label: "Italiano" },
  { code: "es", label: "Español" },
  { code: "fr", label: "Français" },
  { code: "de", label: "Deutsch" },
  { code: "pt", label: "Português" },
];

async function refreshShows() {
  shows.value = await remote.fetchAdminShows();
}

async function selectShow(id) {
  selectedShow.value = await remote.fetchAdminShow(id);
}

async function createDraft() {
  isBusy.value = true;
  try {
    const show = await remote.createShowDraft({
      brief: draftBrief.value,
      targetDurationMinutes: targetMinutes.value,
      language: language.value,
    });
    draftBrief.value = "";
    selectedShow.value = show;
    await refreshShows();
  } finally {
    isBusy.value = false;
  }
}

async function saveScript() {
  if (!selectedShow.value) return;
  isBusy.value = true;
  try {
    selectedShow.value = await remote.updateShowScript(selectedShow.value);
    await refreshShows();
  } finally {
    isBusy.value = false;
  }
}

async function synthesize() {
  if (!selectedShow.value) return;
  isBusy.value = true;
  try {
    await remote.synthesizeShow(selectedShow.value.id);
    selectedShow.value = await remote.fetchAdminShow(selectedShow.value.id);
    await refreshShows();
  } finally {
    isBusy.value = false;
  }
}

async function publish() {
  if (!selectedShow.value) return;
  isBusy.value = true;
  try {
    selectedShow.value = await remote.publishShow(selectedShow.value.id);
    await refreshShows();
  } finally {
    isBusy.value = false;
  }
}

async function deleteCurrent() {
  if (!selectedShow.value) return;
  const id = selectedShow.value.id;
  isBusy.value = true;
  try {
    await remote.deleteShow(id);
    selectedShow.value = null;
    await refreshShows();
  } finally {
    isBusy.value = false;
  }
}

onMounted(refreshShows);
</script>

<style scoped>
.showsManager {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-5);
}

.draftPanel,
.editorPanel,
.emptyEditor {
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--bg-elevated-base);
  padding: var(--spacing-4);
}

.draftPanel h2,
.editorPanel h3 {
  margin: 0 0 var(--spacing-2);
}

.draftPanel p,
.emptyState,
.emptyEditor,
.trackRef {
  color: var(--text-subdued);
}

.draftForm,
.segments,
.speakers {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

textarea,
input,
select {
  width: 100%;
  box-sizing: border-box;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--bg-base);
  color: var(--text-base);
  padding: 10px;
  font: inherit;
}

.draftControls,
.actions,
.metaRow,
.speakerRow,
.segmentHeader {
  display: flex;
  gap: var(--spacing-2);
  align-items: center;
}

.draftControls label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 140px;
}

.draftControls label span {
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  text-transform: uppercase;
}

.draftControls input,
.draftControls select {
  max-width: 160px;
}

.layout {
  display: grid;
  grid-template-columns: 280px minmax(0, 1fr);
  gap: var(--spacing-4);
}

.showList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.showListItem {
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: left;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--bg-elevated-base);
  color: var(--text-base);
  padding: var(--spacing-3);
  cursor: pointer;
}

.showListItem.active {
  border-color: var(--spotify-green);
  background: rgba(29, 185, 84, 0.12);
}

.showListItem small,
.status {
  color: var(--text-subdued);
}

.editorHeader {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--spacing-4);
  align-items: start;
}

.titleInput {
  font-size: var(--text-xl);
  font-weight: var(--font-bold);
}

.summaryInput {
  margin-top: var(--spacing-2);
}

.actions {
  flex-wrap: wrap;
  justify-content: flex-end;
}

.primaryButton,
.secondaryButton,
.dangerButton {
  border-radius: 8px;
  padding: 10px 14px;
  font-weight: var(--font-bold);
  cursor: pointer;
}

.primaryButton {
  border: 0;
  background: var(--spotify-green);
  color: #000;
}

.secondaryButton {
  border: 1px solid var(--surface-border);
  background: var(--bg-base);
  color: var(--text-base);
}

.dangerButton {
  border: 1px solid #bb3434;
  background: transparent;
  color: #ff8a8a;
}

button:disabled {
  opacity: 0.5;
  cursor: default;
}

.metaRow {
  margin: var(--spacing-4) 0;
  flex-wrap: wrap;
}

.metaRow label {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.metaRow input,
.metaRow select {
  width: 140px;
}

.errorBox {
  border: 1px solid #bb3434;
  border-radius: 8px;
  padding: var(--spacing-3);
  color: #ffb1b1;
  margin-bottom: var(--spacing-4);
}

.segmentEditor {
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  padding: var(--spacing-3);
}

.segmentHeader {
  margin-bottom: var(--spacing-2);
}

.segmentHeader span {
  width: 90px;
  color: var(--spotify-green);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  text-transform: uppercase;
}

@media (max-width: 960px) {
  .layout,
  .editorHeader {
    grid-template-columns: 1fr;
  }

  .actions {
    justify-content: flex-start;
  }
}
</style>
