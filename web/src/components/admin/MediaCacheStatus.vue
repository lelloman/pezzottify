<template>
  <section class="mediaCache" aria-labelledby="media-cache-title">
    <div class="cacheHeader">
      <div>
        <h2 id="media-cache-title">Media cache</h2>
        <p>
          Faster copies of audio and artwork. Original media stays in place.
        </p>
      </div>
      <button type="button" :disabled="loading" @click="load">
        {{ loading ? "Refreshing…" : "Refresh" }}
      </button>
    </div>
    <p v-if="error" class="cacheError" role="alert">{{ error }}</p>
    <p v-if="loading && !report" role="status">Loading cache status…</p>
    <template v-if="report">
      <div class="statusLine">
        <strong :class="{ enabled: report.enabled }">
          {{ report.enabled ? "Enabled" : "Not configured" }}
        </strong>
        <span v-if="updatedAt">Last updated {{ updatedAt }}</span>
      </div>
      <p v-if="!report.enabled" class="emptyState">
        No media cache is configured. Media reads use the original storage.
      </p>
      <template v-else>
        <div class="cacheList">
          <article
            v-for="cache in report.caches"
            :key="cache.name"
            class="cacheCard"
          >
            <div class="cacheHeading">
              <h3>{{ cache.name }}</h3>
              <span>{{
                cache.lru
                  ? "Least recently used eviction"
                  : "No automatic eviction"
              }}</span>
            </div>
            <p class="cachePath">{{ cache.path }}</p>
            <div class="capacityLabel">
              <strong>{{ bytes(cache.allocated_bytes) }} used</strong>
              <span>{{ bytes(cache.max_bytes) }} budget</span>
            </div>
            <div
              class="capacityBar"
              role="progressbar"
              :aria-label="`${cache.name} capacity used or reserved`"
              :aria-valuenow="percent(cache)"
              aria-valuemin="0"
              aria-valuemax="100"
            >
              <span :style="{ width: `${percent(cache)}%` }"></span>
            </div>
            <p class="reservation">
              {{ bytes(cache.reserved_bytes) }} reserved for pending fills
            </p>
            <dl class="cacheStats">
              <div>
                <dt>Completed fills</dt>
                <dd>{{ count(cache.fills) }}</dd>
              </div>
              <div>
                <dt>Evictions</dt>
                <dd>{{ count(cache.evictions) }}</dd>
              </div>
              <div>
                <dt>Fill errors</dt>
                <dd>{{ count(cache.fill_errors) }}</dd>
              </div>
              <div>
                <dt>Eviction errors</dt>
                <dd>{{ count(cache.eviction_errors) }}</dd>
              </div>
            </dl>
          </article>
        </div>
        <p v-if="!report.caches.length">
          This cache does not expose storage statistics.
        </p>
        <h3 class="activityTitle">Read activity across all caches</h3>
        <div class="tableScroll">
          <table>
            <thead>
              <tr>
                <th scope="col">Media</th>
                <th scope="col">Hit rate</th>
                <th scope="col">Hits</th>
                <th scope="col">Misses</th>
                <th scope="col">Fallback reads</th>
                <th scope="col">Cache errors</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="kind in kinds" :key="kind.key">
                <th scope="row">{{ kind.label }}</th>
                <td>{{ ratio(report[kind.key].hit_ratio) }}</td>
                <td>{{ count(report[kind.key].hits) }}</td>
                <td>{{ count(report[kind.key].misses) }}</td>
                <td>{{ count(report[kind.key].fallback_reads) }}</td>
                <td>{{ count(report[kind.key].errors) }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="activityNote">
          Skipped fills: {{ count(report.fills_skipped_capacity) }} at capacity
          · {{ count(report.fills_skipped_busy) }} while busy or unavailable.
        </p>
        <p class="activityNote">
          Hit rate is hits ÷ (hits + fallback reads). — means no reads yet. Read
          errors can also count as misses; fallback reads are attempts to use
          the original retrieval path.
        </p>
      </template>
    </template>
    <p class="refreshNote">
      Counters reset when the server restarts. Refreshes every 15 seconds while
      this panel is visible.
    </p>
  </section>
</template>

<script setup>
import { onMounted, onUnmounted, ref } from "vue";
import axios from "axios";

const report = ref(null);
const loading = ref(false);
const error = ref("");
const updatedAt = ref("");
const kinds = [
  { key: "audio", label: "Audio" },
  { key: "image", label: "Artwork" },
];
let timer;
let controller;
let disposed = false;

const count = (value) => Number(value).toLocaleString();
const ratio = (value) =>
  value === null ? "—" : `${(value * 100).toFixed(1)}%`;
const bytes = (value) => {
  if (!value) return "0 B";
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), 4);
  return `${(value / 1024 ** index).toLocaleString(undefined, { maximumFractionDigits: 1 })} ${["B", "KiB", "MiB", "GiB", "TiB"][index]}`;
};
const percent = (cache) =>
  Math.min(
    100,
    Math.max(
      0,
      (100 * (cache.allocated_bytes + cache.reserved_bytes)) / cache.max_bytes,
    ),
  );

async function load() {
  if (disposed || loading.value || document.hidden) return;
  clearTimeout(timer);
  controller = new AbortController();
  loading.value = true;
  error.value = "";
  let forbidden = false;
  try {
    const response = await axios.get("/v1/admin/media-cache", {
      signal: controller.signal,
    });
    if (disposed) return;
    report.value = response.data;
    updatedAt.value = new Date().toLocaleTimeString();
  } catch (failure) {
    if (disposed) return;
    forbidden = [401, 403].includes(failure.response?.status);
    if (forbidden) report.value = null;
    error.value = forbidden
      ? "Server admin permission is required to view cache status."
      : report.value
        ? "Refresh failed. Showing the last successful snapshot."
        : "Could not load cache status. Try Refresh.";
  } finally {
    loading.value = false;
    if (!disposed && !forbidden) timer = setTimeout(load, 15000);
  }
}
function visibilityChanged() {
  if (document.hidden) clearTimeout(timer);
  else load();
}
onMounted(() => {
  document.addEventListener("visibilitychange", visibilityChanged);
  load();
});
onUnmounted(() => {
  disposed = true;
  clearTimeout(timer);
  controller?.abort();
  document.removeEventListener("visibilitychange", visibilityChanged);
});
</script>

<style scoped>
.mediaCache {
  color: var(--text-base);
  margin: var(--spacing-6) 0;
  padding: var(--spacing-4);
  border-radius: var(--radius-lg);
  background: var(--bg-elevated-base);
}
.cacheHeader,
.cacheHeading,
.capacityLabel,
.statusLine {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--spacing-3);
}
h2,
h3,
p {
  margin: 0;
}
h2 {
  font-size: var(--text-xl);
}
h3 {
  font-size: var(--text-md);
}
.cacheHeader p,
.cacheHeading span,
.statusLine span,
.cachePath,
.reservation,
dt,
.activityNote,
.refreshNote {
  color: var(--text-subdued);
  font-size: var(--text-sm);
}
.cacheHeader p {
  margin-top: var(--spacing-2);
}
button {
  padding: var(--spacing-2) var(--spacing-4);
  background: var(--bg-highlight);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  cursor: pointer;
}
button:disabled {
  opacity: 0.6;
  cursor: wait;
}
button:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
}
.statusLine,
.cacheError,
.emptyState {
  margin-top: var(--spacing-4);
}
.enabled {
  color: var(--spotify-green);
}
.cacheError {
  color: var(--error);
}
.cacheList {
  display: grid;
  gap: var(--spacing-4);
  margin-top: var(--spacing-4);
}
.cacheCard {
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  padding: var(--spacing-4);
}
.cachePath {
  margin: var(--spacing-2) 0 var(--spacing-4);
  overflow-wrap: anywhere;
}
.capacityBar {
  height: 8px;
  background: var(--bg-highlight);
  border-radius: var(--radius-full);
  margin: var(--spacing-2) 0;
  overflow: hidden;
}
.capacityBar span {
  display: block;
  height: 100%;
  background: var(--spotify-green);
}
.cacheStats {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: var(--spacing-3);
  margin: var(--spacing-4) 0 0;
}
dd {
  margin: var(--spacing-1) 0 0;
  font-size: var(--text-lg);
  font-variant-numeric: tabular-nums;
}
.activityTitle {
  margin-top: var(--spacing-5);
}
.tableScroll {
  overflow-x: auto;
  margin-top: var(--spacing-3);
}
table {
  width: 100%;
  border-collapse: collapse;
  white-space: nowrap;
  font-size: var(--text-sm);
}
th,
td {
  text-align: right;
  padding: var(--spacing-3);
  border-bottom: 1px solid var(--border-default);
  font-variant-numeric: tabular-nums;
}
th:first-child {
  text-align: left;
  padding-left: 0;
}
thead th {
  color: var(--text-subdued);
  font-weight: var(--font-medium);
}
.activityNote,
.refreshNote {
  margin-top: var(--spacing-3);
  line-height: 1.5;
}
@media (max-width: 700px) {
  .cacheHeader,
  .cacheHeading,
  .capacityLabel,
  .statusLine {
    align-items: flex-start;
    flex-wrap: wrap;
  }
  .cacheStats {
    grid-template-columns: repeat(2, 1fr);
  }
}
</style>
