<template>
  <div class="homePage">
    <!-- Recently Played Section -->
    <section v-if="recentlyPlayed.length > 0" class="homeSection">
      <h2 class="sectionTitle">Recently Played</h2>
      <div class="albumGrid">
        <router-link
          v-for="item in recentlyPlayed"
          :key="item.album_id"
          :to="`/album/${item.album_id}`"
          class="albumCard"
        >
          <div class="albumCover">
            <img
              v-if="item.album_image_id"
              :src="getImageUrl(item.album_image_id)"
              :alt="item.album_name"
              loading="lazy"
            />
            <div v-else class="placeholderCover">
              <MusicNoteIcon class="placeholderIcon" />
            </div>
          </div>
          <div class="albumInfo">
            <span class="albumName">{{ item.album_name }}</span>
            <span class="artistName">{{ item.artist_name }}</span>
          </div>
        </router-link>
      </div>
    </section>

    <!-- Popular Section -->
    <section v-if="popular.albums?.length > 0" class="homeSection">
      <h2 class="sectionTitle">Popular Albums</h2>
      <div class="albumGrid">
        <router-link
          v-for="album in popular.albums"
          :key="album.id"
          :to="`/album/${album.id}`"
          class="albumCard"
        >
          <div class="albumCover">
            <img
              v-if="album.image_id"
              :src="getImageUrl(album.image_id)"
              :alt="album.name"
              loading="lazy"
            />
            <div v-else class="placeholderCover">
              <MusicNoteIcon class="placeholderIcon" />
            </div>
          </div>
          <div class="albumInfo">
            <span class="albumName">{{ album.name }}</span>
            <span class="artistName">{{ formatArtistNames(album.artist_names) }}</span>
          </div>
        </router-link>
      </div>
    </section>

    <!-- Popular Artists Section -->
    <section v-if="popular.artists?.length > 0" class="homeSection">
      <h2 class="sectionTitle">Popular Artists</h2>
      <div class="artistGrid">
        <router-link
          v-for="artist in popular.artists"
          :key="artist.id"
          :to="`/artist/${artist.id}`"
          class="artistCard"
        >
          <div class="artistImage">
            <img
              v-if="artist.image_id"
              :src="getImageUrl(artist.image_id)"
              :alt="artist.name"
              loading="lazy"
            />
            <div v-else class="placeholderCover round">
              <MusicNoteIcon class="placeholderIcon" />
            </div>
          </div>
          <span class="artistCardName">{{ artist.name }}</span>
        </router-link>
      </div>
    </section>

    <!-- Your Favorites Section -->
    <section v-if="favorites.length > 0" class="homeSection">
      <h2 class="sectionTitle">Your Favorites</h2>
      <div class="albumGrid">
        <router-link
          v-for="album in favorites"
          :key="album.id"
          :to="`/album/${album.id}`"
          class="albumCard"
        >
          <div class="albumCover">
            <img
              v-if="getAlbumImageId(album)"
              :src="getImageUrl(getAlbumImageId(album))"
              :alt="album.name"
              loading="lazy"
            />
            <div v-else class="placeholderCover">
              <MusicNoteIcon class="placeholderIcon" />
            </div>
          </div>
          <div class="albumInfo">
            <span class="albumName">{{ album.name }}</span>
            <span class="artistName">{{ album.artist_name }}</span>
          </div>
        </router-link>
      </div>
    </section>

    <!-- Loading State -->
    <div v-if="isLoading" class="loadingState">
      Loading...
    </div>

    <!-- Empty State -->
    <div v-if="!isLoading && isEmpty" class="emptyState">
      <MusicNoteIcon class="emptyIcon" />
      <h2>Welcome to Pezzottify</h2>
      <p>Start exploring music by searching above</p>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, computed } from "vue";
import MusicNoteIcon from "@/components/icons/MusicNoteIcon.vue";
import { formatImageUrl } from "@/utils";

const getImageUrl = formatImageUrl;

const recentlyPlayed = ref([]);
const popular = ref({ albums: [], artists: [] });
const favorites = ref([]);
const isLoading = ref(true);

const isEmpty = computed(() => {
  return (
    recentlyPlayed.value.length === 0 &&
    popular.value.albums?.length === 0 &&
    popular.value.artists?.length === 0 &&
    favorites.value.length === 0
  );
});

// Helper to format artist names array as string
const formatArtistNames = (names) => {
  if (!names || names.length === 0) return "Unknown Artist";
  return names.join(", ");
};

// Helper to get album cover image ID from album object
const getAlbumImageId = (album) => {
  if (!album) return null;
  // Check covers array first (preferred)
  if (album.covers && album.covers.length > 0) {
    return album.covers[0].id;
  }
  // Fall back to cover_group
  if (album.cover_group && album.cover_group.length > 0) {
    return album.cover_group[0].id;
  }
  return null;
};

const fetchRecentlyPlayed = async () => {
  try {
    const response = await fetch("/v1/user/listening/history?limit=10");
    if (response.ok) {
      const data = await response.json();
      // Deduplicate by album_id, keeping only first occurrence
      const seen = new Set();
      recentlyPlayed.value = data.entries
        .filter((entry) => {
          if (seen.has(entry.album_id)) return false;
          seen.add(entry.album_id);
          return true;
        })
        .slice(0, 8);
    }
  } catch (error) {
    console.error("Error fetching recently played:", error);
  }
};

const fetchPopular = async () => {
  try {
    const response = await fetch("/v1/content/popular?albums_limit=8&artists_limit=8");
    if (response.ok) {
      popular.value = await response.json();
    }
  } catch (error) {
    console.error("Error fetching popular:", error);
  }
};

const fetchFavorites = async () => {
  try {
    // First get the list of liked album IDs
    const response = await fetch("/v1/user/liked/album");
    if (response.ok) {
      const albumIds = await response.json();
      // Then fetch details for each album (limit to 8)
      const albumPromises = albumIds.slice(0, 8).map(async (albumId) => {
        try {
          const albumResponse = await fetch(`/v1/content/album/${albumId}/resolved`);
          if (albumResponse.ok) {
            return await albumResponse.json();
          }
        } catch (e) {
          console.error(`Error fetching album ${albumId}:`, e);
        }
        return null;
      });
      const albums = await Promise.all(albumPromises);
      favorites.value = albums.filter((a) => a !== null);
    }
  } catch (error) {
    console.error("Error fetching favorites:", error);
  }
};

onMounted(async () => {
  await Promise.all([
    fetchRecentlyPlayed(),
    fetchPopular(),
    fetchFavorites(),
  ]);
  isLoading.value = false;
});
</script>

<style scoped>
.homePage {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-6);
}

.homeSection {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.sectionTitle {
  font-size: var(--text-xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
}

.albumGrid,
.artistGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-4);
}

.albumCard,
.artistCard {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  padding: var(--spacing-3);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  text-decoration: none;
  transition: background-color var(--transition-fast);
}

.albumCard:hover,
.artistCard:hover {
  background-color: var(--bg-elevated-highlight);
}

.albumCover,
.artistImage {
  aspect-ratio: 1;
  border-radius: var(--radius-md);
  overflow: hidden;
  background-color: var(--bg-base);
}

.artistImage {
  border-radius: 50%;
}

.albumCover img,
.artistImage img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.placeholderCover {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  background-color: var(--bg-highlight);
}

.placeholderCover.round {
  border-radius: 50%;
}

.placeholderIcon {
  width: 40%;
  height: 40%;
  color: var(--text-subdued);
}

.albumInfo {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.albumName,
.artistCardName {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.artistName {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.artistCard {
  align-items: center;
  text-align: center;
}

.artistCardName {
  width: 100%;
}

.loadingState,
.emptyState {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-8);
  color: var(--text-subdued);
  text-align: center;
  gap: var(--spacing-3);
}

.emptyIcon {
  width: 64px;
  height: 64px;
  color: var(--text-subdued);
  opacity: 0.5;
}

.emptyState h2 {
  margin: 0;
  color: var(--text-base);
}

.emptyState p {
  margin: 0;
}
</style>
