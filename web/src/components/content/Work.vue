<template>
  <article class="workPage" :aria-busy="loading">
    <p v-if="loading && !work" class="statePanel" role="status">
      Loading composition…
    </p>
    <p v-if="error" class="statePanel" role="alert">
      {{ error }}
      <button type="button" :disabled="loading" @click="load">Retry</button>
    </p>
    <template v-if="work">
      <header class="workHeader">
        <WorkArtwork
          class="compositionMark"
          :artistIds="work.creator_artist_ids || []"
        />
        <div class="workIdentity">
          <p class="eyebrow">
            Work<span v-if="work.kind">
              · {{ work.kind.replace(/_/g, " ") }}</span
            >
          </p>
          <h1>{{ work.title }}</h1>
          <div v-if="work.creators?.length" class="credits">
            <p class="creditLabel">Written by</p>
            <p class="creators">{{ work.creators.join(" · ") }}</p>
          </div>
          <p class="workCaption">
            <template v-if="work.composition_year"
              >Composed {{ work.composition_year }} ·
            </template>
            Recordings, parts and related works.
          </p>
        </div>
      </header>

      <section
        v-if="relationGroups.length"
        class="relations"
        aria-label="Work relationships"
      >
        <details
          v-for="group in relationGroups"
          :key="group.label"
          :open="
            group.label === 'Parts & movements' || group.label === 'Part of'
          "
        >
          <summary>
            {{ group.label }} <span>{{ group.items.length }}</span>
          </summary>
          <ul class="relationList">
            <li
              v-for="relation in group.items"
              :key="`${relation.work.id}-${relation.ordering}`"
            >
              <RouterLink
                :to="{ name: 'work', params: { workId: relation.work.id } }"
              >
                <span v-if="relation.ordering > 0" class="partNumber"
                  >{{ relation.ordering }}.</span
                >
                <span
                  >{{ relation.work.title
                  }}<small>{{
                    relation.work.creators.join(" · ")
                  }}</small></span
                >
              </RouterLink>
            </li>
          </ul>
        </details>
      </section>

      <section class="recordings" aria-labelledby="recordings-title">
        <div class="sectionHeading">
          <div>
            <p class="eyebrow">Explore the music</p>
            <h2 id="recordings-title">Recordings &amp; versions</h2>
          </div>
          <span class="recordingCount"
            >{{ tracks.length }}{{ hasMore ? "+" : "" }}
            {{
              tracks.length === 1 && !hasMore ? "recording" : "recordings"
            }}</span
          >
        </div>
        <label class="recordingFilter"
          >Show recordings
          <select v-model="scope" @change="changeScope">
            <option value="all">
              This work, its parts &amp; related works
            </option>
            <option value="parts">This work &amp; its parts</option>
            <option value="related">Related works only</option>
          </select>
        </label>
        <p v-if="!tracks.length && !loading && !error" class="statePanel">
          {{
            scope === "related"
              ? "No recordings linked to related works yet."
              : scope === "parts"
                ? "No recordings linked to this work or its parts yet."
                : "No recordings linked to this work, its parts or related works yet."
          }}
        </p>
        <div v-else class="recordingList">
          <div class="listHeading" aria-hidden="true">
            <span>Recording / artist</span><span>Album</span>
          </div>
          <div
            v-for="(entry, index) in tracks"
            :key="entry.track.id"
            class="recordingRow"
          >
            <div class="recordingTrack">
              <LoadTrackListItem
                :trackId="entry.track.id"
                :trackNumber="index + 1"
                :isCurrentlyPlaying="playback.currentTrackId === entry.track.id"
                @track-clicked="playback.setTrack($event)"
                @track-image-clicked="
                  router.push({
                    name: 'track',
                    params: { trackId: entry.track.id },
                  })
                "
              />
              <p
                v-if="
                  entry.recording_work && entry.relationship_scope !== 'direct'
                "
                class="recordingWork"
              >
                {{
                  entry.relationship_scope === "part" ? "Part" : "Related work"
                }}
                ·
                <RouterLink
                  :to="{
                    name: 'work',
                    params: { workId: entry.recording_work.id },
                  }"
                  >{{ entry.recording_work.title }}</RouterLink
                >
              </p>
            </div>
            <RouterLink
              v-if="entry.album"
              class="albumLink"
              :to="{ name: 'album', params: { albumId: entry.album.id } }"
              >{{ entry.album.name }}</RouterLink
            >
          </div>
        </div>
        <div v-if="hasMore && !error" class="pagination">
          <button type="button" :disabled="loading" @click="load">
            {{ loading ? "Loading…" : "Load more recordings" }}
          </button>
        </div>
      </section>

      <details
        v-if="work.catalog_number || work.musicbrainz_id || work.wikidata_id"
        class="workDetails"
      >
        <summary>About this work <span>Catalog &amp; sources</span></summary>
        <div class="detailsBody">
          <dl v-if="work.catalog_number">
            <dt>Catalog number</dt>
            <dd>{{ work.catalog_number }}</dd>
          </dl>
          <p v-if="work.musicbrainz_id">
            <a
              :href="`https://musicbrainz.org/work/${work.musicbrainz_id}`"
              target="_blank"
              rel="noopener noreferrer"
            >
              MusicBrainz reference
            </a>
          </p>
          <p v-if="work.wikidata_id">
            <a
              :href="`https://www.wikidata.org/wiki/${work.wikidata_id}`"
              target="_blank"
              rel="noopener noreferrer"
              >Wikidata reference</a
            >
          </p>
        </div>
      </details>
    </template>
  </article>
</template>

<script setup>
import { computed, ref, watch, onBeforeUnmount } from "vue";
import { useRouter } from "vue-router";
import WorkArtwork from "@/components/common/WorkArtwork.vue";
import LoadTrackListItem from "@/components/common/LoadTrackListItem.vue";
import { usePlaybackStore } from "@/store/playback";
import axios from "axios";
const props = defineProps({ workId: { type: String, required: true } });
const router = useRouter();
const playback = usePlaybackStore();
const work = ref(null);
const tracks = ref([]);
const relations = ref([]);
const scope = ref("all");
const relationGroups = computed(() => {
  const groups = new Map();
  for (const relation of relations.value) {
    const label = relationLabel(relation);
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label).push(relation);
  }
  return [...groups].map(([label, items]) => ({ label, items }));
});
function relationLabel({ relationship_type: type, direction }) {
  const outgoing = direction === "outgoing";
  const labels = {
    parts: ["Parts & movements", "Part of"],
    arrangement: ["Arrangements", "Arrangement of"],
    orchestration: ["Orchestrations", "Orchestration of"],
    "based on": ["Works based on this", "Based on"],
    "other version": ["Other versions", "Version of"],
    adaptation: ["Adaptations", "Adaptation of"],
    "revision of": ["Revisions", "Revision of"],
    "included works": ["Included works", "Included in"],
    medley: ["Medley of", "Included in medleys"],
    "musical quotation": ["Quotes music from", "Music quoted in"],
    "lyrical quotation": ["Quotes lyrics from", "Lyrics quoted in"],
    "named after work": ["Named after", "Inspired the name of"],
  };
  return (
    labels[type]?.[outgoing ? 0 : 1] ||
    `${type} (${outgoing ? "outgoing" : "incoming"})`
  );
}
function changeScope() {
  controller?.abort();
  tracks.value = [];
  hasMore.value = false;
  loading.value = false;
  offset = 0;
  load();
}
const loading = ref(false);
const error = ref("");
const hasMore = ref(false);
let offset = 0;
let controller;
async function load() {
  if (loading.value) return;
  const request = new AbortController();
  controller = request;
  loading.value = true;
  error.value = "";
  try {
    const { data } = await axios.get(
      `/v1/content/work/${encodeURIComponent(props.workId)}`,
      {
        params: { limit: 50, offset, scope: scope.value },
        signal: request.signal,
      },
    );
    if (request.signal.aborted) return;
    work.value = data.work;
    relations.value = data.relations || [];
    tracks.value.push(...data.tracks);
    hasMore.value = data.has_more;
    offset = data.next_offset;
  } catch (e) {
    if (request.signal.aborted) return;
    error.value =
      e.response?.status === 404
        ? "Work not found."
        : work.value
          ? "Could not load more recordings. Your loaded recordings are still available."
          : "Could not load this work.";
  } finally {
    if (controller === request) loading.value = false;
  }
}
watch(
  () => props.workId,
  () => {
    controller?.abort();
    work.value = null;
    relations.value = [];
    scope.value = "all";
    tracks.value = [];
    hasMore.value = false;
    loading.value = false;
    offset = 0;
    load();
  },
  { immediate: true },
);
onBeforeUnmount(() => controller?.abort());
</script>

<style scoped>
.workPage {
  color: var(--text-base);
  padding-bottom: 32px;
}
.workHeader {
  display: grid;
  grid-template-columns: 200px minmax(0, 1fr);
  gap: clamp(24px, 4vw, 48px);
  align-items: center;
  padding: clamp(24px, 4vw, 44px);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: linear-gradient(125deg, rgba(29, 185, 84, 0.18), transparent 65%),
    var(--surface-raised);
}
.compositionMark {
  aspect-ratio: 1;
  width: 100%;
  font-size: 40px;
}
.workIdentity {
  min-width: 0;
}
.eyebrow {
  color: var(--text-subdued);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
  margin: 0 0 10px;
}
h1 {
  font-size: clamp(36px, 5vw, 68px);
  font-weight: 800;
  line-height: 1.07;
  letter-spacing: -0.04em;
  margin: 0;
  overflow-wrap: anywhere;
}
.credits {
  margin-top: 24px;
}
.creditLabel,
dt {
  color: var(--text-subdued);
  font-size: 12px;
  margin: 0 0 6px;
}
.creators {
  font-size: 16px;
  line-height: 1.7;
  margin: 0;
  overflow-wrap: anywhere;
}
.workCaption {
  color: var(--text-subdued);
  font-size: 13px;
  margin: 20px 0 0;
}
.recordings {
  margin-top: 36px;
}
.relations {
  margin-top: 24px;
}
.relations details {
  border-bottom: 1px solid var(--surface-border);
}
.relations summary span {
  display: inline;
  margin: 0 0 0 12px;
}
.relationList {
  list-style: none;
  padding: 0;
  margin: 0 0 16px;
}
.relationList a {
  display: flex;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 6px;
}
.relationList a:hover {
  background: var(--surface-hover);
}
.relationList small {
  display: block;
  color: var(--text-subdued);
  margin-top: 4px;
}
.partNumber {
  flex: 0 0 24px;
  color: var(--text-subdued);
}
.recordingFilter {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  color: var(--text-subdued);
  margin: 0 8px 20px;
  font-size: 13px;
}
.recordingFilter select {
  max-width: 100%;
  padding: 8px;
  background: var(--surface-raised);
  color: var(--text-base);
  border: 1px solid var(--surface-border);
  border-radius: 6px;
}
.recordingWork {
  margin: 0 12px 8px 48px;
  color: var(--text-subdued);
  font-size: 12px;
  overflow-wrap: anywhere;
}
.sectionHeading {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 16px;
  padding: 0 8px 20px;
}
h2 {
  font-size: clamp(21px, 2.5vw, 28px);
  line-height: 1.25;
  margin: 0;
  letter-spacing: -0.025em;
}
.recordingCount {
  color: var(--text-subdued);
  font-size: 12px;
  flex-shrink: 0;
  padding-bottom: 3px;
}
.recordingList {
  border-top: 1px solid var(--surface-border);
}
.listHeading,
.recordingRow {
  display: grid;
  grid-template-columns: minmax(0, 3fr) minmax(0, 1fr);
  gap: 20px;
  align-items: center;
}
.listHeading {
  padding: 14px 8px;
  color: var(--text-subdued);
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}
.recordingRow {
  border-top: 1px solid var(--surface-border);
  padding: 6px 0;
  border-radius: 6px;
}
.recordingRow:hover {
  background: var(--surface-hover);
}
.recordingRow > * {
  min-width: 0;
}
.albumLink {
  color: var(--text-subdued);
  font-size: 13px;
  overflow-wrap: anywhere;
  margin-right: 12px;
}
a {
  color: inherit;
  text-decoration: none;
}
a:hover {
  color: var(--text-base);
  text-decoration: underline;
}
.pagination {
  display: flex;
  justify-content: center;
  padding: 24px;
}
button {
  background: var(--surface-raised);
  border: 1px solid var(--surface-border-strong);
  border-radius: 999px;
  padding: 10px 20px;
  color: var(--text-base);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
button:hover {
  background: var(--surface-hover);
}
button:disabled {
  opacity: 0.5;
  cursor: wait;
}
button:focus-visible,
a:focus-visible,
summary:focus-visible {
  outline: 2px solid var(--accent-color);
  outline-offset: 4px;
}
.statePanel {
  padding: 28px;
  background: var(--surface-panel);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  color: var(--text-subdued);
  line-height: 1.6;
}
.workDetails {
  border-top: 1px solid var(--surface-border);
  margin: 32px 8px 0;
}
summary {
  padding: 20px 0;
  font-size: 14px;
  cursor: pointer;
}
summary span {
  color: var(--text-subdued);
  font-size: 12px;
  margin-left: 12px;
}
.detailsBody {
  display: flex;
  align-items: center;
  gap: 24px;
  flex-wrap: wrap;
  padding: 0 0 16px;
}
dl {
  margin: 0;
}
dd {
  margin: 0;
  font-size: 14px;
  overflow-wrap: anywhere;
}
.detailsBody a {
  display: inline-block;
  padding: 8px 12px;
  border: 1px solid var(--surface-border);
  border-radius: 6px;
  font-size: 13px;
}
@media (max-width: 700px) {
  .workHeader {
    grid-template-columns: 1fr;
    gap: 20px;
  }
  .compositionMark {
    width: 96px;
  }
  .compositionMark span {
    font-size: 6px;
    margin-bottom: 6px;
  }
  .sectionHeading {
    align-items: start;
    flex-direction: column;
    gap: 10px;
  }
  .recordingRow {
    grid-template-columns: minmax(0, 1fr);
    gap: 0;
  }
  .listHeading {
    display: none;
  }
  .albumLink {
    margin: 0 12px 8px 56px;
    font-size: 12px;
  }
  .recordingRow :deep(.track-item-content) {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) auto;
    gap: 4px 8px;
  }
  .recordingRow :deep(.trackIndexSpan) {
    grid-column: 1;
    grid-row: 1 / 3;
    width: auto;
    padding: 0;
    text-align: center;
  }
  .recordingRow :deep(.trackImage) {
    display: none;
  }
  .recordingRow :deep(.trackNameSpan) {
    grid-column: 2;
    grid-row: 1;
    width: auto;
    min-width: 0;
    margin: 0;
  }
  .recordingRow :deep(.trackArtistsSpan) {
    grid-column: 2;
    grid-row: 2;
    width: auto;
    padding: 0;
    font-size: 12px;
    color: var(--text-subdued);
  }
  .recordingRow :deep(.track-duration) {
    grid-column: 3;
    grid-row: 1 / 3;
    font-size: 12px;
    color: var(--text-subdued);
  }
  summary span {
    display: block;
    margin: 8px 0 0;
  }
}
</style>
