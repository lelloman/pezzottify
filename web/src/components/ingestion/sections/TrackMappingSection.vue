<template>
  <div class="track-mapping">
    <table class="mapping-table">
      <thead>
        <tr>
          <th class="col-file">File</th>
          <th class="col-track">Matched Track</th>
          <th class="col-confidence">Match</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="file in files" :key="file.id" :class="{ matched: file.matched_track_id }">
          <td class="col-file">
            <span class="filename" :title="file.filename">{{ truncateFilename(file.filename) }}</span>
            <span v-if="file.tag_title" class="tag-title">{{ file.tag_title }}</span>
          </td>
          <td class="col-track">
            <span v-if="file.matched_track_id" class="track-matched">
              &#10003; Track #{{ file.tag_track_num || '?' }}
            </span>
            <span v-else class="track-pending">-</span>
          </td>
          <td class="col-confidence">
            <span v-if="file.match_confidence" class="confidence">
              {{ Math.round(file.match_confidence * 100) }}%
            </span>
            <span v-else>-</span>
          </td>
        </tr>
      </tbody>
    </table>

    <div v-if="files.length === 0" class="no-files">
      No files to display
    </div>
  </div>
</template>

<script setup>
defineProps({
  files: {
    type: Array,
    default: () => [],
  },
});

function truncateFilename(name) {
  if (!name) return "-";
  const base = name.replace(/\.[^.]+$/, "");
  if (base.length > 30) {
    return base.substring(0, 27) + "...";
  }
  return base;
}
</script>

<style scoped>
.track-mapping {
  font-size: 13px;
}

.mapping-table {
  width: 100%;
  border-collapse: collapse;
}

.mapping-table th {
  text-align: left;
  padding: 8px 4px;
  border-bottom: 1px solid var(--border-default);
  color: var(--text-subdued);
  font-weight: 500;
  font-size: 12px;
}

.mapping-table td {
  padding: 8px 4px;
  border-bottom: 1px solid var(--bg-highlight);
}

.col-file {
  width: 50%;
}

.col-track {
  width: 35%;
}

.col-confidence {
  width: 15%;
  text-align: right;
}

.filename {
  display: block;
  color: var(--text-base);
}

.tag-title {
  display: block;
  font-size: 11px;
  color: var(--text-subdued);
}

tr.matched {
  background: rgba(29, 185, 84, 0.1);
}

.track-matched {
  color: var(--spotify-green);
}

.track-pending {
  color: var(--text-subtle);
}

.confidence {
  color: var(--text-base);
}

.no-files {
  padding: 20px;
  text-align: center;
  color: var(--text-subdued);
}
</style>
