<template>
  <div class="homePage">
    <div v-if="isLoading" class="loadingState">
      <div class="loadingPulse"></div>
      <span>Loading library</span>
    </div>

    <template v-else-if="!isEmpty">
      <section v-if="heroAlbum" class="heroSection">
        <router-link :to="`/album/${heroAlbum.id}`" class="heroArtwork">
          <img
            :src="getImageUrl(heroAlbum.id)"
            :alt="heroAlbum.name"
            loading="eager"
          />
        </router-link>
        <div class="heroCopy">
          <span class="eyebrow">Featured from your library</span>
          <router-link :to="`/album/${heroAlbum.id}`" class="heroTitle">
            {{ heroAlbum.name }}
          </router-link>
          <p class="heroMeta">
            {{ formatArtistNames(heroAlbum.artist_names) }}
          </p>
          <div class="heroActions">
            <router-link :to="`/album/${heroAlbum.id}`" class="primaryAction"
              >Open album</router-link
            >
            <router-link
              v-if="genres.length"
              to="/genres"
              class="secondaryAction"
              >Browse genres</router-link
            >
          </div>
        </div>
        <div class="heroStats">
          <div v-if="popular.albums?.length" class="statBlock">
            <strong>{{ popular.albums.length }}</strong>
            <span>popular albums</span>
          </div>
          <div v-if="popular.artists?.length" class="statBlock">
            <strong>{{ popular.artists.length }}</strong>
            <span>artists trending</span>
          </div>
          <div v-if="genres.length" class="statBlock">
            <strong>{{ genres.length }}</strong>
            <span>top genres</span>
          </div>
        </div>
      </section>

      <section
        v-if="recentlyPlayed.length > 0"
        class="homeSection recentlyPlayedSection"
      >
        <div class="sectionHeader">
          <div>
            <span class="sectionKicker">Pick up where you left off</span>
            <h2 class="sectionTitle">Recently Played</h2>
          </div>
        </div>
        <div class="recentGrid">
          <router-link
            v-for="item in recentlyPlayed"
            :key="item.album_id"
            :to="`/album/${item.album_id}`"
            class="recentCard"
          >
            <div class="recentCover">
              <img
                :src="getImageUrl(item.album_id)"
                :alt="item.album_name"
                loading="lazy"
              />
            </div>
            <div class="recentInfo">
              <span class="recentName">{{ item.album_name }}</span>
              <span class="recentArtist">{{ item.artist_name }}</span>
            </div>
          </router-link>
        </div>
      </section>

      <section v-if="popular.albums?.length > 0" class="homeSection">
        <div class="sectionHeader">
          <div>
            <span class="sectionKicker">Heavy rotation</span>
            <h2 class="sectionTitle">Popular Albums</h2>
          </div>
        </div>
        <div class="albumShelf">
          <router-link
            v-for="album in popular.albums"
            :key="album.id"
            :to="`/album/${album.id}`"
            class="albumCard"
          >
            <div class="albumCover">
              <img
                :src="getImageUrl(album.id)"
                :alt="album.name"
                loading="lazy"
              />
            </div>
            <div class="albumInfo">
              <span class="albumName">{{ album.name }}</span>
              <span class="artistName">{{
                formatArtistNames(album.artist_names)
              }}</span>
            </div>
          </router-link>
        </div>
      </section>

      <section
        v-if="popular.artists?.length > 0"
        class="homeSection artistSection"
      >
        <div class="sectionHeader">
          <div>
            <span class="sectionKicker">Most played voices</span>
            <h2 class="sectionTitle">Popular Artists</h2>
          </div>
        </div>
        <div class="artistList">
          <router-link
            v-for="(artist, index) in popular.artists"
            :key="artist.id"
            :to="`/artist/${artist.id}`"
            class="artistRow"
          >
            <span class="artistRank">{{
              String(index + 1).padStart(2, "0")
            }}</span>
            <div class="artistImage">
              <img
                :src="getImageUrl(artist.id)"
                :alt="artist.name"
                loading="lazy"
              />
            </div>
            <span class="artistCardName">{{ artist.name }}</span>
          </router-link>
        </div>
      </section>

      <section v-if="genres.length > 0" class="homeSection genreSection">
        <div class="sectionHeader">
          <div>
            <span class="sectionKicker">Explore by mood and catalog depth</span>
            <h2 class="sectionTitle">Browse Genres</h2>
          </div>
          <router-link to="/genres" class="seeAllLink">See all</router-link>
        </div>
        <div class="genreGrid">
          <router-link
            v-for="genre in genres"
            :key="genre.name"
            :to="`/genre/${encodeURIComponent(genre.name)}`"
            class="genreCard"
          >
            <span class="genreCardName">{{ genre.name }}</span>
            <span class="genreTrackCount">{{
              formatTrackCount(genre.track_count)
            }}</span>
          </router-link>
        </div>
      </section>

      <section v-if="favorites.length > 0" class="homeSection">
        <div class="sectionHeader">
          <div>
            <span class="sectionKicker">Saved for later</span>
            <h2 class="sectionTitle">Your Favorites</h2>
          </div>
        </div>
        <div class="albumShelf compactShelf">
          <router-link
            v-for="album in favorites"
            :key="album.id"
            :to="`/album/${album.id}`"
            class="albumCard"
          >
            <div class="albumCover">
              <img
                :src="getImageUrl(album.id)"
                :alt="album.name"
                loading="lazy"
              />
            </div>
            <div class="albumInfo">
              <span class="albumName">{{ album.name }}</span>
              <span class="artistName">{{ album.artist_name }}</span>
            </div>
          </router-link>
        </div>
      </section>
    </template>

    <div v-else class="emptyState">
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
const genres = ref([]);
const isLoading = ref(true);

const heroAlbum = computed(() => {
  const popularAlbum = popular.value.albums?.[0];
  if (popularAlbum) return popularAlbum;

  const recentAlbum = recentlyPlayed.value[0];
  if (recentAlbum) {
    return {
      id: recentAlbum.album_id,
      name: recentAlbum.album_name,
      artist_names: recentAlbum.artist_name ? [recentAlbum.artist_name] : [],
    };
  }

  const favoriteAlbum = favorites.value[0];
  if (favoriteAlbum) {
    return {
      id: favoriteAlbum.id,
      name: favoriteAlbum.name,
      artist_names:
        favoriteAlbum.artist_names ||
        (favoriteAlbum.artist_name ? [favoriteAlbum.artist_name] : []),
    };
  }

  return null;
});

const isEmpty = computed(() => {
  return (
    recentlyPlayed.value.length === 0 &&
    popular.value.albums?.length === 0 &&
    popular.value.artists?.length === 0 &&
    favorites.value.length === 0 &&
    genres.value.length === 0
  );
});

// Helper to format artist names array as string
const formatArtistNames = (names) => {
  if (!names || names.length === 0) return "Unknown Artist";
  return names.join(", ");
};

// Helper to format track count
const formatTrackCount = (count) => {
  if (count === 1) return "1 track";
  return `${count.toLocaleString()} tracks`;
};

const fetchRecentlyPlayed = async () => {
  try {
    const response = await fetch("/v1/user/listening/history?limit=10");
    if (response.ok) {
      const data = await response.json();
      // Deduplicate by album_id, keeping only first occurrence
      const seen = new Set();
      const entries = Array.isArray(data.entries) ? data.entries : [];
      recentlyPlayed.value = entries
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
    const response = await fetch(
      "/v1/content/popular?albums_limit=8&artists_limit=8",
    );
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
          const albumResponse = await fetch(
            `/v1/content/album/${albumId}/resolved`,
          );
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

const fetchGenres = async () => {
  try {
    const response = await fetch("/v1/content/genres");
    if (response.ok) {
      const allGenres = await response.json();
      // Show top 12 genres by track count
      genres.value = allGenres.slice(0, 12);
    }
  } catch (error) {
    console.error("Error fetching genres:", error);
  }
};

onMounted(async () => {
  await Promise.all([
    fetchRecentlyPlayed(),
    fetchPopular(),
    fetchFavorites(),
    fetchGenres(),
  ]);
  isLoading.value = false;
});
</script>

<style scoped>
.homePage {
  display: flex;
  flex-direction: column;
  gap: 32px;
  min-height: 100%;
  padding: clamp(18px, 2vw, 30px);
  color: var(--text-base);
}

.heroSection {
  display: grid;
  grid-template-columns: minmax(150px, 220px) minmax(0, 1fr) minmax(
      150px,
      210px
    );
  gap: clamp(18px, 2.4vw, 32px);
  align-items: end;
  min-height: 300px;
  padding: clamp(18px, 3vw, 34px);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: linear-gradient(
      135deg,
      rgba(29, 185, 84, 0.22),
      rgba(21, 24, 22, 0.2) 36%,
      rgba(18, 18, 18, 0.88)
    ),
    radial-gradient(
      circle at 86% 10%,
      rgba(244, 178, 71, 0.22),
      transparent 34%
    ),
    #151515;
  overflow: hidden;
}

.heroArtwork {
  display: block;
  aspect-ratio: 1;
  border-radius: 8px;
  overflow: hidden;
  background: #242424;
  box-shadow: 0 22px 48px rgba(0, 0, 0, 0.45);
}

.heroArtwork img,
.albumCover img,
.recentCover img,
.artistImage img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.heroCopy {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 10px;
}

.eyebrow,
.sectionKicker {
  color: #9eddb7;
  font-size: 0.72rem;
  font-weight: 800;
  letter-spacing: 0;
  text-transform: uppercase;
}

.heroTitle {
  color: #fff;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  font-size: clamp(2rem, 4.2vw, 4.8rem);
  font-weight: 900;
  line-height: 0.94;
  letter-spacing: 0;
  text-decoration: none;
}

.heroTitle:hover {
  color: #fff;
}

.heroMeta {
  color: rgba(255, 255, 255, 0.74);
  font-size: 0.98rem;
  max-width: 720px;
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.heroActions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-top: 8px;
}

.primaryAction,
.secondaryAction {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 40px;
  padding: 0 18px;
  border-radius: 999px;
  font-size: 0.9rem;
  font-weight: 800;
  text-decoration: none;
}

.primaryAction {
  background: #1ed760;
  color: #071108;
}

.primaryAction:hover {
  background: #35e473;
  color: #071108;
}

.secondaryAction {
  color: #fff;
  border: 1px solid rgba(255, 255, 255, 0.18);
  background: rgba(255, 255, 255, 0.06);
}

.secondaryAction:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.1);
}

.heroStats {
  display: grid;
  gap: 10px;
  align-self: stretch;
  align-content: end;
}

.statBlock {
  padding: 14px;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.26);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

.statBlock strong,
.statBlock span {
  display: block;
}

.statBlock strong {
  font-size: 1.45rem;
  font-weight: 900;
  line-height: 1;
}

.statBlock span {
  margin-top: 4px;
  color: rgba(255, 255, 255, 0.64);
  font-size: 0.76rem;
  font-weight: 650;
}

.homeSection {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.sectionHeader {
  display: flex;
  justify-content: space-between;
  align-items: end;
  gap: 16px;
}

.sectionTitle {
  margin: 2px 0 0;
  color: #fff;
  font-size: clamp(1.15rem, 1.7vw, 1.65rem);
  font-weight: 900;
  line-height: 1.15;
}

.seeAllLink {
  flex: 0 0 auto;
  color: rgba(255, 255, 255, 0.62);
  font-size: 0.82rem;
  font-weight: 800;
  text-decoration: none;
}

.seeAllLink:hover {
  color: #fff;
}

.albumShelf {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 18px;
}

.albumCard {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 11px;
  color: #fff;
  text-decoration: none;
}

.albumCover {
  aspect-ratio: 1;
  border-radius: 8px;
  overflow: hidden;
  background: #242424;
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.24);
  transition:
    transform var(--transition-base),
    filter var(--transition-base),
    box-shadow var(--transition-base);
}

.albumCard:hover .albumCover {
  transform: translateY(-4px);
  filter: brightness(1.08);
  box-shadow: 0 18px 34px rgba(0, 0, 0, 0.34);
}

.albumInfo,
.recentInfo {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 3px;
}

.albumName,
.recentName,
.artistCardName {
  color: #fff;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 850;
}

.albumName {
  font-size: 0.88rem;
}

.artistName,
.recentArtist,
.genreTrackCount {
  color: rgba(255, 255, 255, 0.58);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.76rem;
  font-weight: 620;
}

.recentGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
  gap: 10px;
}

.recentCard {
  display: grid;
  grid-template-columns: 58px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  min-height: 74px;
  padding: 8px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.045);
  border: 1px solid rgba(255, 255, 255, 0.055);
  color: #fff;
  text-decoration: none;
  transition:
    background var(--transition-fast),
    border-color var(--transition-fast);
}

.recentCard:hover {
  color: #fff;
  background: rgba(255, 255, 255, 0.08);
  border-color: rgba(255, 255, 255, 0.12);
}

.recentCover {
  width: 58px;
  height: 58px;
  border-radius: 6px;
  overflow: hidden;
  background: #222;
}

.artistList {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
  gap: 10px;
}

.artistRow {
  display: grid;
  grid-template-columns: 32px 52px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
  min-height: 68px;
  padding: 8px 12px 8px 8px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  border: 1px solid rgba(255, 255, 255, 0.055);
  color: #fff;
  text-decoration: none;
}

.artistRow:hover {
  color: #fff;
  background: rgba(29, 185, 84, 0.12);
  border-color: rgba(29, 185, 84, 0.22);
}

.artistRank {
  color: rgba(255, 255, 255, 0.36);
  font-size: 0.82rem;
  font-weight: 900;
  text-align: center;
}

.artistImage {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  overflow: hidden;
  background: #242424;
}

.artistCardName {
  font-size: 0.9rem;
}

.genreGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 10px;
}

.genreCard {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  min-height: 92px;
  padding: 14px;
  border-radius: 8px;
  color: #fff;
  text-decoration: none;
  background: linear-gradient(
      135deg,
      rgba(255, 255, 255, 0.075),
      rgba(255, 255, 255, 0.025)
    ),
    #181818;
  border: 1px solid rgba(255, 255, 255, 0.065);
  transition:
    transform var(--transition-fast),
    border-color var(--transition-fast),
    background var(--transition-fast);
}

.genreCard:nth-child(3n + 1) {
  background: linear-gradient(
      135deg,
      rgba(29, 185, 84, 0.2),
      rgba(255, 255, 255, 0.025)
    ),
    #181818;
}

.genreCard:nth-child(3n + 2) {
  background: linear-gradient(
      135deg,
      rgba(58, 134, 255, 0.2),
      rgba(255, 255, 255, 0.025)
    ),
    #181818;
}

.genreCard:nth-child(3n) {
  background: linear-gradient(
      135deg,
      rgba(255, 180, 84, 0.2),
      rgba(255, 255, 255, 0.025)
    ),
    #181818;
}

.genreCard:hover {
  color: #fff;
  transform: translateY(-2px);
  border-color: rgba(255, 255, 255, 0.16);
}

.genreCardName {
  color: #fff;
  font-size: 0.98rem;
  font-weight: 900;
  line-height: 1.15;
  text-transform: capitalize;
}

.loadingState,
.emptyState {
  min-height: 55vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: rgba(255, 255, 255, 0.62);
  text-align: center;
}

.loadingPulse {
  width: 44px;
  height: 44px;
  border-radius: 50%;
  border: 3px solid rgba(255, 255, 255, 0.1);
  border-top-color: #1ed760;
  animation: spin 0.9s linear infinite;
}

.emptyIcon {
  width: 64px;
  height: 64px;
  color: rgba(255, 255, 255, 0.42);
}

.emptyState h2 {
  margin: 0;
  font-weight: 900;
}

.emptyState p {
  margin: 0;
  color: rgba(255, 255, 255, 0.62);
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 1180px) {
  .heroSection {
    grid-template-columns: minmax(130px, 180px) minmax(0, 1fr);
  }

  .heroStats {
    grid-column: 1 / -1;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .homePage {
    padding: 14px;
    gap: 26px;
  }

  .heroSection {
    grid-template-columns: 1fr;
    min-height: auto;
  }

  .heroArtwork {
    width: min(68vw, 240px);
  }

  .heroStats {
    grid-template-columns: 1fr;
  }

  .recentGrid,
  .artistList {
    grid-template-columns: 1fr;
  }

  .albumShelf {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
  }

  .genreGrid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
</style>
