<template>
  <div class="album-match">
    <div v-if="job?.matched_album_id" class="matched-album">
      <div class="match-header">
        <span class="match-icon">&#10003;</span>
        <span class="match-label">Matched Album</span>
      </div>
      <div class="match-details">
        <div class="album-info">
          <span class="album-name">{{ job?.detected_album || 'Unknown Album' }}</span>
          <span class="artist-name">{{ job?.detected_artist || 'Unknown Artist' }}</span>
        </div>
        <div class="confidence-bar">
          <div
            class="confidence-fill"
            :style="{ width: confidencePercent + '%' }"
            :class="confidenceClass"
          ></div>
        </div>
        <span class="confidence-label">{{ confidencePercent }}% match</span>
      </div>
    </div>

    <div v-if="candidates.length > 0" class="candidates">
      <div class="candidates-header">Top Candidates</div>
      <div v-for="candidate in candidates" :key="candidate.id" class="candidate">
        <div class="candidate-info">
          <span class="candidate-name">{{ candidate.name }}</span>
          <span class="candidate-artist">{{ candidate.artist_name }}</span>
        </div>
        <div class="candidate-stats">
          <span class="candidate-tracks">{{ candidate.track_count }} tracks</span>
          <div class="candidate-score-bar">
            <div
              class="score-fill"
              :style="{ width: (candidate.score * 100) + '%' }"
            ></div>
          </div>
          <span class="candidate-score">{{ Math.round(candidate.score * 100) }}%</span>
        </div>
      </div>
    </div>

    <div v-if="!job?.matched_album_id && candidates.length === 0" class="no-match">
      <span class="no-match-icon">?</span>
      <span>No album match yet</span>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";

const props = defineProps({
  job: {
    type: Object,
    default: null,
  },
  candidates: {
    type: Array,
    default: () => [],
  },
});

const confidencePercent = computed(() => {
  const conf = props.job?.match_confidence;
  if (!conf) return 0;
  return Math.round(conf * 100);
});

const confidenceClass = computed(() => {
  const pct = confidencePercent.value;
  if (pct >= 90) return "high";
  if (pct >= 70) return "medium";
  return "low";
});
</script>

<style scoped>
.album-match {
  font-size: 13px;
}

.matched-album {
  margin-bottom: 16px;
}

.match-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.match-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: var(--spotify-green);
  color: var(--text-negative);
  border-radius: 50%;
  font-size: 12px;
  font-weight: bold;
}

.match-label {
  font-weight: 500;
  color: var(--text-base);
}

.match-details {
  padding-left: 28px;
}

.album-info {
  margin-bottom: 8px;
}

.album-name {
  display: block;
  font-weight: 500;
  color: var(--text-base);
}

.artist-name {
  display: block;
  color: var(--text-subdued);
  font-size: 12px;
}

.confidence-bar {
  height: 6px;
  background: var(--bg-highlight);
  border-radius: 3px;
  overflow: hidden;
  margin-bottom: 4px;
}

.confidence-fill {
  height: 100%;
  transition: width 0.3s ease;
}

.confidence-fill.high {
  background: var(--spotify-green);
}

.confidence-fill.medium {
  background: #f5a623;
}

.confidence-fill.low {
  background: #d0021b;
}

.confidence-label {
  font-size: 12px;
  color: var(--text-subdued);
}

.candidates {
  border-top: 1px solid var(--border-default);
  padding-top: 12px;
}

.candidates-header {
  font-weight: 500;
  margin-bottom: 8px;
  color: var(--text-base);
}

.candidate {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--bg-highlight);
}

.candidate:last-child {
  border-bottom: none;
}

.candidate-info {
  flex: 1;
}

.candidate-name {
  display: block;
  color: var(--text-base);
}

.candidate-artist {
  display: block;
  font-size: 12px;
  color: var(--text-subdued);
}

.candidate-stats {
  display: flex;
  align-items: center;
  gap: 8px;
}

.candidate-tracks {
  font-size: 12px;
  color: var(--text-subdued);
}

.candidate-score-bar {
  width: 60px;
  height: 4px;
  background: var(--bg-highlight);
  border-radius: 2px;
  overflow: hidden;
}

.score-fill {
  height: 100%;
  background: #4a90d9;
}

.candidate-score {
  width: 36px;
  text-align: right;
  font-size: 12px;
  color: var(--text-base);
}

.no-match {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-subdued);
}

.no-match-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: var(--bg-highlight);
  color: var(--text-subdued);
  border-radius: 50%;
  font-size: 12px;
  font-weight: bold;
}
</style>
