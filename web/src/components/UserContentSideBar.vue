<template>
  <aside class="panel">
    <div class="tabSelectorsContainer">
      <div
        @click.stop="setAlbumsTab"
        :class="{
          tabSelector: true,
          scaleClickFeedback: true,
          selectedTab: selectedTab === 'albums',
        }"
      >
        <h3>Albums</h3>
      </div>
      <div
        @click.stop="setArtistsTab"
        :class="{
          tabSelector: true,
          scaleClickFeedback: true,
          selectedTab: selectedTab === 'artists',
        }"
      >
        <h3>Artists</h3>
      </div>
      <div
        @click.stop="setPlaylistsTab"
        :class="{
          tabSelector: true,
          scaleClickFeedback: true,
          selectedTab: selectedTab === 'playlists',
        }"
      >
        <h3>Playlists</h3>
      </div>
    </div>
    <div v-if="selectedTab == 'albums'" class="contentContainer">
      <div v-for="albumId in albumIds" :key="albumId">
        <AlbumCard :albumId="albumId" :showArtists="true" />
      </div>
    </div>
    <div v-else-if="selectedTab == 'artists'" class="contentContainer">
      <div v-for="artistId in artistsIds" :key="artistId">
        <LoadArtistListItem :artistId="artistId" />
      </div>
    </div>
    <div v-else-if="selectedTab == 'playlists'" class="contentContainer">
      <div class="createPlaylistButtonContainer">
        <div
          class="createPlaylistButton scaleClickFeedback"
          @click.stop="handleCreatePlaylistButtonClick"
        >
          <span v-if="!isCreatingPlaylist">Create</span>
          <span v-else>...</span>
        </div>
      </div>
      <div class="playlistsContainer" v-if="playlistsData">
        <div v-for="playlist in playlists" :key="playlist.id">
          <LoadPlaylistListItem :playlistId="playlist.id" />
        </div>
      </div>
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
.tabSelectorsContainer {
  display: flex;
  gap: 6px;
  padding: 8px;
  margin: 0;
  border-bottom: 1px solid var(--surface-border);
}

.tabSelector {
  cursor: pointer;
  flex: 1;
  min-width: 0;
  padding: 9px 10px;
  border-radius: 7px;
  transition:
    background-color var(--transition-fast),
    opacity var(--transition-fast);
  opacity: 0.62;
  text-align: center;
}

.tabSelector > h3 {
  color: var(--text-base);
  font-size: 0.78rem;
  font-weight: 850;
  white-space: nowrap;
}

.tabSelector:hover {
  background-color: var(--surface-hover);
  transition:
    scale 0.3s ease,
    background-color 0.3s ease,
    opacity 0.3s ease;
  opacity: 1;
}

.tabSelector:active {
  transition:
    scale 0.3s ease,
    opacity 0.3s ease;
  opacity: 1;
}

.selectedTab {
  background-color: var(--surface-active) !important;
  color: var(--spotify-green);
  transition: transform;
  opacity: 1 !important;
}

.selectedTab > h3 {
  color: var(--spotify-green);
}

.contentContainer {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  gap: 4px;
}

.playlistsContainer {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.createPlaylistButton {
  margin: 8px;
  border: 1px solid var(--surface-border);
  border-radius: 7px;
  padding: 9px 14px;
  width: fit-content;
  cursor: pointer;
  transition: background-color 0.3s ease;
}

.createPlaylistButton:hover {
  background-color: var(--surface-hover);
  transition: background-color 0.3s ease;
}
</style>
