<template>
  <div class="detailPageHost" :aria-busy="loading">
    <p v-if="loading && !work" class="statePanel" role="status">
      Loading composition…
    </p>
    <p v-if="error" class="statePanel" role="alert">
      {{ error }}
      <button type="button" :disabled="loading" @click="load">Retry</button>
    </p>
    <DetailPage
      v-if="work"
      :title="work.title"
      kind="Composition"
      tinted
      :imageUrls="
        work.creator_artist_ids?.length
          ? [formatImageUrl(work.creator_artist_ids[0])]
          : []
      "
    >
      <template #artwork
        ><WorkArtwork :artistIds="work.creator_artist_ids || []"
      /></template>
      <template #meta>
        <div class="workMeta">
          <span v-if="work.creators?.length"
            >Written by {{ work.creators.join(" · ") }}</span
          >
          <template v-if="work.composition_year">
            <span v-if="work.creators?.length" aria-hidden="true">•</span>
            <span>{{ work.composition_year }}</span>
          </template>
          <template v-if="work.catalog_number">
            <span
              v-if="work.creators?.length || work.composition_year"
              aria-hidden="true"
              >•</span
            >
            <span>{{ work.catalog_number }}</span>
          </template>
        </div>
      </template>
      <template #actions
        ><DetailActions
          playLabel="Play recordings"
          @play="playback.setWorkVersions(work.id, undefined, work.title)"
      /></template>
      <section class="recordings" aria-labelledby="recordings-title">
        <div class="sectionHeading">
          <div>
            <h2 id="recordings-title">Recordings &amp; versions</h2>
          </div>
          <span class="recordingCount"
            >{{ tracks.length }}{{ hasMore ? "+" : "" }}
            {{
              tracks.length === 1 && !hasMore ? "recording" : "recordings"
            }}</span
          >
        </div>
        <div class="recordingFilter" role="group" aria-label="Recording scope">
          <button
            v-for="option in scopeOptions"
            :key="option.value"
            type="button"
            :aria-pressed="scope === option.value"
            :title="option.description"
            @click="selectScope(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
        <p
          v-if="loading && !tracks.length"
          class="loadingRecordings"
          role="status"
        >
          Loading recordings…
        </p>
        <p v-if="!tracks.length && !loading && !error" class="statePanel">
          {{
            scope === "related"
              ? "No recordings linked to related works yet."
              : scope === "parts"
                ? "No recordings linked to this work or its parts yet."
                : "No recordings linked to this work, its parts or related works yet."
          }}
        </p>
        <div v-if="tracks.length" class="recordingList">
          <div class="detailTrackHeading" aria-hidden="true">
            <span>#</span><span>Title</span><span>Duration</span>
          </div>
          <div
            v-for="(entry, index) in tracks"
            :key="entry.track.id"
            class="recordingRow"
            :class="{
              currentRecording: playback.currentTrackId === entry.track.id,
            }"
            @contextmenu.prevent="
              trackMenu?.openMenu(
                $event,
                'track',
                entry.track.id,
                entry.track.name,
              )
            "
          >
            <div class="recordingTrack">
              <LoadTrackListItem
                albumLayout
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
              <div
                v-if="
                  entry.album ||
                  (entry.recording_work &&
                    entry.relationship_scope !== 'direct')
                "
                class="recordingContext"
              >
                <RouterLink
                  v-if="entry.album"
                  class="albumLink"
                  :to="{ name: 'album', params: { albumId: entry.album.id } }"
                  >{{ entry.album.name }}</RouterLink
                >
                <span
                  v-if="
                    entry.recording_work &&
                    entry.relationship_scope !== 'direct'
                  "
                  class="recordingWork"
                >
                  <span v-if="entry.album" aria-hidden="true"> · </span>
                  {{
                    entry.relationship_scope === "part"
                      ? "Part"
                      : "Related work"
                  }}
                  ·
                  <RouterLink
                    :to="{
                      name: 'work',
                      params: { workId: entry.recording_work.id },
                    }"
                    >{{ entry.recording_work.title }}</RouterLink
                  >
                </span>
              </div>
            </div>
          </div>
        </div>
        <div v-if="hasMore && !error" class="pagination">
          <button type="button" :disabled="loading" @click="load">
            {{ loading ? "Loading…" : "Load more recordings" }}
          </button>
        </div>
      </section>

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
      <EntityContextMenu ref="trackMenu" />
    </DetailPage>
  </div>
</template>

<script setup>
import DetailPage from "@/components/common/DetailPage.vue";
import { formatImageUrl } from "@/utils";
import EntityContextMenu from "@/components/common/contextmenu/EntityContextMenu.vue";
import DetailActions from "@/components/common/DetailActions.vue";

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
const trackMenu = ref(null);
const scopeOptions = [
  {
    value: "all",
    label: "All recordings",
    description: "This work, its parts & related works",
  },
  {
    value: "parts",
    label: "Work & parts",
    description: "This work & its parts",
  },
  {
    value: "related",
    label: "Related works",
    description: "Related works only",
  },
];
function selectScope(value) {
  if (scope.value === value) return;
  scope.value = value;
  changeScope();
}
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
.workMeta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px 8px;
}
.sectionHeading {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px 16px;
  margin-bottom: 20px;
}
h2 {
  font-size: var(--text-2xl);
  font-weight: 700;
  letter-spacing: -0.02em;
  margin: 0;
}
.recordingCount,
.loadingRecordings {
  color: var(--text-subdued);
  font-size: var(--text-sm);
}
.recordingFilter {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 24px;
}
.recordingFilter button {
  padding: 8px 12px;
  border: 0;
  background: #ffffff12;
  font-size: var(--text-sm);
}
.recordingFilter button:hover {
  background: #ffffff20;
}
.recordingFilter button[aria-pressed="true"] {
  background: #fff;
  color: #000;
}
.recordingList .detailTrackHeading {
  margin-bottom: 8px;
}
.recordingRow {
  min-width: 0;
  border-radius: 4px;
  padding-bottom: 8px;
}
.recordingRow:hover,
.recordingRow:focus-within {
  background: #ffffff12;
}
.recordingRow.currentRecording {
  background: var(--surface-active);
}
.recordingRow :deep(.albumTrackRow.playingTrack),
.recordingRow :deep(.albumTrackRow:hover) {
  background: transparent;
}
.recordingContext {
  margin: 0 16px 0 48px;
  color: var(--text-subdued);
  font-size: 12px;
  line-height: 20px;
  overflow-wrap: anywhere;
}
a {
  color: inherit;
  text-decoration: none;
}
a:hover {
  color: var(--text-base);
  text-decoration: underline;
}
.relations,
.workDetails {
  margin-top: 32px;
}
.relations details,
.workDetails {
  border-top: 1px solid #ffffff15;
}
summary {
  padding: 20px 0;
  font-size: 1rem;
  font-weight: 700;
  cursor: pointer;
}
summary span {
  margin-left: 12px;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-weight: 400;
}
.relationList {
  list-style: none;
  padding: 0;
  margin: 0 0 16px;
}
.relationList a {
  display: flex;
  gap: 16px;
  padding: 12px 16px;
  border-radius: 4px;
}
.relationList a:hover {
  background: #ffffff12;
}
.relationList small {
  display: block;
  color: var(--text-subdued);
  margin-top: 4px;
  font-size: var(--text-sm);
}
.partNumber {
  flex: 0 0 16px;
  color: var(--text-subdued);
}
.detailsBody {
  display: flex;
  gap: 24px;
  align-items: center;
  flex-wrap: wrap;
  padding-bottom: 16px;
  font-size: var(--text-sm);
}
dl,
dd {
  margin: 0;
}
dt {
  color: var(--text-subdued);
  margin-bottom: 4px;
}
dd {
  overflow-wrap: anywhere;
}
.pagination {
  padding: 24px 0;
}
button {
  background: transparent;
  border: 1px solid #ffffff50;
  border-radius: 999px;
  padding: 10px 20px;
  color: var(--text-base);
  font: inherit;
  font-size: var(--text-sm);
  cursor: pointer;
}
button:hover {
  border-color: #fff;
}
button:disabled {
  opacity: 0.5;
  cursor: wait;
}
button:focus-visible,
a:focus-visible,
summary:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 3px;
}
.statePanel {
  padding: 24px;
  color: var(--text-subdued);
  line-height: 1.6;
}
@container (max-width: 560px) {
  .sectionHeading {
    align-items: start;
    flex-direction: column;
  }
  summary span {
    display: block;
    margin: 8px 0 0;
  }
}
</style>
