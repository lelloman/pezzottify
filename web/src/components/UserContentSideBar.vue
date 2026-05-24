<template>
  <aside class="panel libraryPanel">
    <div class="tabSelectorsContainer">
      <button
        type="button"
        @click.stop="setAlbumsTab"
        :class="{
          tabSelector: true,
          scaleClickFeedback: true,
          selectedTab: selectedTab === 'albums',
        }"
      >
        <span>Albums</span>
      </button>
      <button
        type="button"
        @click.stop="setArtistsTab"
        :class="{
          tabSelector: true,
          scaleClickFeedback: true,
          selectedTab: selectedTab === 'artists',
        }"
      >
        <span>Artists</span>
      </button>
      <button
        type="button"
        @click.stop="setPlaylistsTab"
        :class="{
          tabSelector: true,
          scaleClickFeedback: true,
          selectedTab: selectedTab === 'playlists',
        }"
      >
        <span>Playlists</span>
      </button>
    </div>

    <div v-if="selectedTab == 'albums'" class="contentContainer">
      <div v-if="loading" class="libraryState">Loading library</div>
      <template v-else-if="albumIds?.length">
        <AlbumCard
          v-for="albumId in albumIds"
          :key="albumId"
          :albumId="albumId"
          :showArtists="true"
        />
      </template>
      <div v-else class="libraryState">No saved albums</div>
    </div>

    <div v-else-if="selectedTab == 'artists'" class="contentContainer">
      <div v-if="loading" class="libraryState">Loading library</div>
      <template v-else-if="artistsIds?.length">
        <LoadArtistListItem
          v-for="artistId in artistsIds"
          :key="artistId"
          :artistId="artistId"
        />
      </template>
      <div v-else class="libraryState">No saved artists</div>
    </div>

    <div v-else-if="selectedTab == 'playlists'" class="contentContainer">
      <button
        type="button"
        class="createPlaylistButton scaleClickFeedback"
        :disabled="isCreatingPlaylist"
        @click.stop="handleCreatePlaylistButtonClick"
      >
        <PlusIcon class="createPlaylistIcon" />
        <span v-if="!isCreatingPlaylist">New playlist</span>
        <span v-else>Creating</span>
      </button>

      <div v-if="loading" class="libraryState">Loading library</div>
      <div v-else-if="playlists.length" class="playlistsContainer">
        <LoadPlaylistListItem
          v-for="playlist in playlists"
          :key="playlist.id"
          :playlistId="playlist.id"
        />
      </div>
      <div v-else class="libraryState">No playlists</div>
    </div>
  </aside>
</template>

<script setup>
import "@/assets/base.css";
import "@/assets/main.css";
import { watch, ref, onMounted, computed } from "vue";
import { useUserStore } from "@/store/user.js";
import { useRouter } from "vue-router";
import AlbumCard from "@/components/common/AlbumCard.vue";
import LoadArtistListItem from "@/components/common/LoadArtistListItem.vue";
import LoadPlaylistListItem from "./common/LoadPlaylistListItem.vue";
import PlusIcon from "@/components/icons/PlusIcon.vue";

const userStore = useUserStore();
const router = useRouter();

const albumIds = ref(null);
const artistsIds = ref(null);
const playlistsData = ref(null);
const loading = ref(true);

const selectedTab = ref(null);

const isCreatingPlaylist = ref(false);

// Get playlists as a computed property to ensure reactivity
const playlists = computed(() => {
  if (playlistsData.value && playlistsData.value.list) {
    // Return the playlist list directly
    return playlistsData.value.list;
  }
  return [];
});

watch(
  () => userStore.isInitializing,
  (newIsInitializing) => {
    loading.value = newIsInitializing;
  },
  { immediate: true },
);
watch(
  () => userStore.likedAlbumIds,
  (likedAlbums) => {
    if (likedAlbums) {
      albumIds.value = likedAlbums;
    }
  },
  { immediate: true },
);
watch(
  () => userStore.likedArtistsIds,
  (likedArtists) => {
    if (likedArtists) {
      artistsIds.value = likedArtists;
    }
  },
  { immediate: true },
);
watch(
  () => userStore.playlistsData,
  (newPlaylistsData) => {
    console.log("new userStore.playlistsData ", newPlaylistsData);
    if (newPlaylistsData) {
      playlistsData.value = newPlaylistsData;
    }
  },
  { immediate: true },
);

const handleCreatePlaylistButtonClick = () => {
  if (isCreatingPlaylist.value) {
    return;
  }
  isCreatingPlaylist.value = true;
  userStore.createPlaylist((newPlaylistId) => {
    isCreatingPlaylist.value = false;
    console.log("New playlist created: ", newPlaylistId);
    if (newPlaylistId) {
      router.push(`/playlist/${newPlaylistId}?edit=true`);
    }
  });
};

const setTab = (tabName) => {
  if (["albums", "artists", "playlists"].indexOf(tabName) < 0) {
    return false;
  }
  selectedTab.value = tabName;
  localStorage.setItem("selectedTab", tabName);
  return true;
};

const setAlbumsTab = () => {
  setTab("albums");
};

const setArtistsTab = () => {
  setTab("artists");
};

const setPlaylistsTab = () => {
  setTab("playlists");
};

onMounted(() => {
  if (!setTab(localStorage.getItem("selectedTab"))) {
    setAlbumsTab();
  }
});
</script>

<style scoped>
.libraryPanel {
  min-height: 0;
  overflow: hidden;
}

.tabSelectorsContainer {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 6px;
  padding: 10px;
  margin: 0;
  border-bottom: 1px solid var(--surface-border);
  background: rgba(255, 255, 255, 0.018);
}

.tabSelector {
  appearance: none;
  cursor: pointer;
  min-width: 0;
  min-height: 36px;
  padding: 0 10px;
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  color: var(--text-subdued);
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast),
    opacity var(--transition-fast);
  opacity: 0.82;
  text-align: center;
}

.tabSelector > span {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 0.78rem;
  font-weight: 850;
}

.tabSelector:hover {
  background-color: var(--surface-hover);
  border-color: var(--surface-border);
  color: var(--text-base);
  opacity: 1;
}

.selectedTab {
  background-color: var(--surface-active) !important;
  border-color: rgba(29, 185, 84, 0.32);
  color: var(--spotify-green);
  opacity: 1 !important;
}

.contentContainer {
  display: flex;
  flex: 1;
  min-height: 0;
  flex-direction: column;
  overflow-y: auto;
  padding: 10px;
  gap: 6px;
  scrollbar-gutter: stable;
}

.playlistsContainer {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.createPlaylistButton {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 7px;
  min-height: 38px;
  margin: 0 0 6px;
  border: 1px solid var(--surface-border);
  border-radius: 7px;
  padding: 0 12px;
  width: 100%;
  background: rgba(255, 255, 255, 0.045);
  color: var(--text-base);
  cursor: pointer;
  font-size: 0.84rem;
  font-weight: 850;
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast);
}

.createPlaylistButton:hover:not(:disabled) {
  background-color: var(--surface-hover);
  border-color: rgba(29, 185, 84, 0.28);
  color: var(--spotify-green);
}

.createPlaylistButton:disabled {
  cursor: wait;
  opacity: 0.65;
}

.createPlaylistIcon {
  width: 18px;
  height: 18px;
  fill: currentColor;
}

.libraryState {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 92px;
  padding: 14px;
  border: 1px dashed var(--surface-border);
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 0.84rem;
  font-weight: 700;
  text-align: center;
}
</style>
