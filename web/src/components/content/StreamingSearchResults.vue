<template>
  <div class="wrapper">
    <!-- Loading indicator -->
    <div v-if="isLoading && sections.length === 0" class="loadingSection">
      <span class="loadingText">Searching...</span>
    </div>

    <!-- Primary Match Section -->
    <div v-if="primaryMatch" class="section primaryMatchSection">
      <h2 class="sectionTitle">Best Match</h2>
      <div class="primaryMatchContainer">
        <div class="primaryMatchItem">
          <AlbumResult v-if="primaryMatch.item.type === 'Album'" :result="primaryMatch.item" />
          <ArtistResult v-else-if="primaryMatch.item.type === 'Artist'" :result="primaryMatch.item" />
          <TrackResult v-else-if="primaryMatch.item.type === 'Track'" :result="primaryMatch.item" />
        </div>
        <div class="confidenceBadge" :class="confidenceClass">
          {{ confidenceLabel }}
        </div>
      </div>
    </div>

    <!-- Top Results Section (when no primary match) -->
    <div v-if="topResults && topResults.items.length > 0" class="section">
      <h2 class="sectionTitle">Top Results</h2>
      <div class="resultsGrid">
        <div v-for="(result, index) in topResults.items" :key="'top-' + index" class="resultItem">
          <AlbumResult v-if="result.type === 'Album'" :result="result" />
          <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
          <TrackResult v-else-if="result.type === 'Track'" :result="result" />
        </div>
      </div>
    </div>

    <!-- Popular Tracks Section -->
    <div v-if="popularBy && popularBy.items.length > 0" class="section">
      <h2 class="sectionTitle">Popular Tracks</h2>
      <div class="tracksList">
        <TrackSummaryRow
          v-for="(track, index) in popularBy.items"
          :key="'popular-' + index"
          :track="track"
        />
      </div>
    </div>

    <!-- Albums Section -->
    <div v-if="albumsBy && albumsBy.items.length > 0" class="section">
      <h2 class="sectionTitle">Albums</h2>
      <div class="albumsGrid">
        <AlbumSummaryCard
          v-for="(album, index) in albumsBy.items"
          :key="'album-' + index"
          :album="album"
        />
      </div>
    </div>

    <!-- Tracks From Album Section -->
    <div v-if="tracksFrom && tracksFrom.items.length > 0" class="section">
      <h2 class="sectionTitle">Tracks from Album</h2>
      <div class="tracksList">
        <TrackSummaryRow
          v-for="(track, index) in tracksFrom.items"
          :key="'track-' + index"
          :track="track"
        />
      </div>
    </div>

    <!-- Related Artists Section -->
    <div v-if="relatedArtists && relatedArtists.items.length > 0" class="section">
      <h2 class="sectionTitle">Related Artists</h2>
      <div class="artistsGrid">
        <ArtistSummaryCard
          v-for="(artist, index) in relatedArtists.items"
          :key="'artist-' + index"
          :artist="artist"
        />
      </div>
    </div>

    <!-- Other Results Section -->
    <div v-if="otherResults && otherResults.items.length > 0" class="section">
      <h2 class="sectionTitle">Other Results</h2>
      <div class="resultsGrid">
        <div v-for="(result, index) in otherResults.items" :key="'other-' + index" class="resultItem">
          <AlbumResult v-if="result.type === 'Album'" :result="result" />
          <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
          <TrackResult v-else-if="result.type === 'Track'" :result="result" />
        </div>
      </div>
    </div>

    <!-- No results -->
    <div v-if="isDone && !hasAnyResults" class="noResults">
      <p>No results found</p>
    </div>

    <!-- Search time -->
    <div v-if="doneSection" class="searchTime">
      Search completed in {{ doneSection.total_time_ms }}ms
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import AlbumResult from "@/components/search/AlbumResult.vue";
import ArtistResult from "@/components/search/ArtistResult.vue";
import TrackResult from "@/components/search/TrackResult.vue";
import TrackSummaryRow from "@/components/search/TrackSummaryRow.vue";
import AlbumSummaryCard from "@/components/search/AlbumSummaryCard.vue";
import ArtistSummaryCard from "@/components/search/ArtistSummaryCard.vue";
import { SectionType } from "@/services/streamingSearch";

const props = defineProps({
  sections: {
    type: Array,
    required: true,
  },
  isLoading: {
    type: Boolean,
    default: false,
  },
});

// Extract sections by type
const primaryMatch = computed(() => {
  return props.sections.find((s) => s.section === SectionType.PRIMARY_MATCH);
});

const topResults = computed(() => {
  return props.sections.find((s) => s.section === SectionType.TOP_RESULTS);
});

const popularBy = computed(() => {
  return props.sections.find((s) => s.section === SectionType.POPULAR_BY);
});

const albumsBy = computed(() => {
  return props.sections.find((s) => s.section === SectionType.ALBUMS_BY);
});

const tracksFrom = computed(() => {
  return props.sections.find((s) => s.section === SectionType.TRACKS_FROM);
});

const relatedArtists = computed(() => {
  return props.sections.find((s) => s.section === SectionType.RELATED_ARTISTS);
});

const otherResults = computed(() => {
  return props.sections.find((s) => s.section === SectionType.OTHER_RESULTS);
});

const doneSection = computed(() => {
  return props.sections.find((s) => s.section === SectionType.DONE);
});

const isDone = computed(() => {
  return doneSection.value !== undefined;
});

const hasAnyResults = computed(() => {
  return (
    primaryMatch.value ||
    (topResults.value && topResults.value.items.length > 0) ||
    (otherResults.value && otherResults.value.items.length > 0)
  );
});

const confidenceClass = computed(() => {
  if (!primaryMatch.value) return "";
  const confidence = primaryMatch.value.confidence;
  if (confidence >= 0.9) return "confidenceHigh";
  if (confidence >= 0.7) return "confidenceMedium";
  return "confidenceLow";
});

const confidenceLabel = computed(() => {
  if (!primaryMatch.value) return "";
  const matchType = primaryMatch.value.match_type;
  return matchType.charAt(0).toUpperCase() + matchType.slice(1);
});
</script>

<style scoped>
.wrapper {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.loadingSection {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 32px;
}

.loadingText {
  color: var(--text-subdued);
  font-style: italic;
}

.section {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.sectionTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0;
}

/* Primary Match */
.primaryMatchSection {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: 16px;
}

.primaryMatchContainer {
  display: flex;
  align-items: center;
  gap: 16px;
}

.primaryMatchItem {
  flex: 1;
}

.confidenceBadge {
  padding: 4px 12px;
  border-radius: 12px;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  white-space: nowrap;
}

.confidenceHigh {
  background-color: var(--accent-color);
  color: white;
}

.confidenceMedium {
  background-color: var(--bg-elevated-highlight);
  color: var(--text-base);
}

.confidenceLow {
  background-color: var(--bg-elevated-base);
  color: var(--text-subdued);
}

/* Results Grid */
.resultsGrid {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(1, 1fr);
}

@media (min-width: 1200px) {
  .resultsGrid {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 1600px) {
  .resultsGrid {
    grid-template-columns: repeat(3, 1fr);
  }
}

.resultItem {
  min-width: 300px;
}

/* Tracks List */
.tracksList {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* Albums Grid */
.albumsGrid {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
}

/* Artists Grid */
.artistsGrid {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
}

/* No Results */
.noResults {
  color: var(--text-subdued);
  font-style: italic;
  text-align: center;
  padding: 32px;
}

/* Search Time */
.searchTime {
  color: var(--text-subdued);
  font-size: var(--text-xs);
  text-align: right;
}
</style>
