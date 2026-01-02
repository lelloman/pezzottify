<template>
  <div class="wrapper">
    <!-- Loading indicator -->
    <div v-if="isLoading && sections.length === 0" class="loadingSection">
      <span class="loadingText">Searching...</span>
    </div>

    <!-- Primary Artist Section -->
    <div v-if="primaryArtist" class="section primarySection">
      <h2 class="sectionTitle">Artist</h2>
      <div class="primaryContainer">
        <ArtistResult :result="primaryArtist.item" />
      </div>
    </div>

    <!-- Artist Enrichment: Popular Tracks -->
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

    <!-- Artist Enrichment: Albums -->
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

    <!-- Artist Enrichment: Related Artists -->
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

    <!-- Primary Album Section -->
    <div v-if="primaryAlbum" class="section primarySection">
      <h2 class="sectionTitle">Album</h2>
      <div class="primaryContainer">
        <AlbumResult :result="primaryAlbum.item" />
      </div>
    </div>

    <!-- Album Enrichment: Tracks From -->
    <div v-if="tracksFrom && tracksFrom.items.length > 0" class="section">
      <h2 class="sectionTitle">Tracks</h2>
      <div class="tracksList">
        <TrackSummaryRow
          v-for="(track, index) in tracksFrom.items"
          :key="'track-' + index"
          :track="track"
        />
      </div>
    </div>

    <!-- Primary Track Section -->
    <div v-if="primaryTrack" class="section primarySection">
      <h2 class="sectionTitle">Track</h2>
      <div class="primaryContainer">
        <TrackResult :result="primaryTrack.item" />
      </div>
    </div>

    <!-- More Results Section (when there are primary matches) -->
    <div v-if="moreResults && moreResults.items.length > 0" class="section">
      <h2 class="sectionTitle">More Results</h2>
      <div class="resultsGrid">
        <div v-for="(result, index) in moreResults.items" :key="'more-' + index" class="resultItem">
          <AlbumResult v-if="result.type === 'Album'" :result="result" />
          <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
          <TrackResult v-else-if="result.type === 'Track'" :result="result" />
        </div>
      </div>
    </div>

    <!-- Results Section (when there are no primary matches) -->
    <div v-if="results && results.items.length > 0" class="section">
      <div class="resultsGrid">
        <div v-for="(result, index) in results.items" :key="'result-' + index" class="resultItem">
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
const primaryArtist = computed(() => {
  return props.sections.find((s) => s.section === SectionType.PRIMARY_ARTIST);
});

const primaryAlbum = computed(() => {
  return props.sections.find((s) => s.section === SectionType.PRIMARY_ALBUM);
});

const primaryTrack = computed(() => {
  return props.sections.find((s) => s.section === SectionType.PRIMARY_TRACK);
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

const moreResults = computed(() => {
  return props.sections.find((s) => s.section === SectionType.MORE_RESULTS);
});

const results = computed(() => {
  return props.sections.find((s) => s.section === SectionType.RESULTS);
});

const doneSection = computed(() => {
  return props.sections.find((s) => s.section === SectionType.DONE);
});

const isDone = computed(() => {
  return doneSection.value !== undefined;
});

const hasAnyResults = computed(() => {
  return (
    primaryArtist.value ||
    primaryAlbum.value ||
    primaryTrack.value ||
    (moreResults.value && moreResults.value.items.length > 0) ||
    (results.value && results.value.items.length > 0)
  );
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

/* Primary sections */
.primarySection {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: 16px;
}

.primaryContainer {
  display: flex;
  align-items: center;
  gap: 16px;
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
