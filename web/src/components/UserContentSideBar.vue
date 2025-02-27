<template>
  <aside class="sidebar panel">
    <div class="tabSelectorsContainer">
      <div @click.stop="setAlbumsTab"
        :class="{ 'tabSelector': true, 'scaleClickFeedback': true, 'selectedTab': selectedTab === 'albums' }">
        <h3>Albums</h3>
      </div>
      <div @click.stop="setArtistsTab"
        :class="{ 'tabSelector': true, 'scaleClickFeedback': true, 'selectedTab': selectedTab === 'artists' }">
        <h3>Artists</h3>
      </div>
      <div @click.stop="setPlaylistsTab"
        :class="{ 'tabSelector': true, 'scaleClickFeedback': true, 'selectedTab': selectedTab === 'playlists' }">
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
        <div class="createPlaylistButton scaleClickFeedback" @click.stop="handleCreatePlaylistButtonClick">
          <span v-if="!isCreatingPlaylist">Create</span>
          <span v-else>...</span>
        </div>
      </div>
      <div class="playlistsContainer" v-if="playlistsData">
        <div v-for="playlistData in playlistsData.list" :key="playlistData">
          <LoadPlaylistListItem :playlistData="playlistData" />
        </div>
      </div>
    </div>

  </aside>
</template>

<script setup>
import '@/assets/base.css';
import '@/assets/main.css';
import { watch, ref, onMounted } from 'vue';
import { useUserStore } from '@/store/user.js';
import { useRouter } from 'vue-router';
import AlbumCard from '@/components/common/AlbumCard.vue';
import LoadArtistListItem from '@/components/common/LoadArtistListItem.vue';
import LoadPlaylistListItem from './common/LoadPlaylistListItem.vue';

const userStore = useUserStore();
const router = useRouter();

const albumIds = ref(null);
const artistsIds = ref(null);
const playlistsData = ref(null);
const loading = ref(true);

const selectedTab = ref(null);

const isCreatingPlaylist = ref(false);

watch([() => userStore.isLoadingLikedAlbums, userStore.isLoadingLikedArtists, userStore.isLoadingPlaylists],
  ([isLoadingLikedAlbums, isLoadingLikedArtists, isLoadingPlaylists]) => {
    loading.value = isLoadingLikedAlbums || isLoadingLikedArtists || isLoadingPlaylists;
  },
  { immediate: true }
);
watch(() => userStore.likedAlbumIds,
  (likedAlbums) => {
    if (likedAlbums) {
      albumIds.value = likedAlbums;
    }
  },
  { immediate: true }
);
watch(() => userStore.likedArtistsIds,
  (likedArtists) => {
    if (likedArtists) {
      artistsIds.value = likedArtists;
    }
  },
  { immediate: true }
);
watch(() => userStore.playlistsData,
  (newPlaylistsData) => {
    console.log("new userStore.playlistsData ", newPlaylistsData);
    if (newPlaylistsData) {
      playlistsData.value = newPlaylistsData;
    }
  },
  { immediate: true }
);

const handleCreatePlaylistButtonClick = () => {
  if (isCreatingPlaylist.value) {
    return;
  }
  isCreatingPlaylist.value = true;
  userStore.createPlaylist((newPlaylistId) => {
    isCreatingPlaylist.value = false;

    if (newPlaylistId) {
      router.push(`/playlist/${newPlaylistId}?edit=true`);
    }
  });
}

const setTab = (tabName) => {
  if (['albums', 'artists', 'playlists'].indexOf(tabName) < 0) {
    return false;
  }
  selectedTab.value = tabName;
  const localStorageMethod = "trigger" + tabName.charAt(0).toUpperCase() + tabName.slice(1) + "Load";
  userStore[localStorageMethod]();
  localStorage.setItem('selectedTab', tabName);
  return true;
};

const setAlbumsTab = () => {
  setTab('albums');
};

const setArtistsTab = () => {
  setTab('artists');
};

const setPlaylistsTab = () => {
  setTab('playlists');
};

onMounted(() => {
  if (!setTab(localStorage.getItem('selectedTab'))) {
    setAlbumsTab();
  }
});
</script>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  min-width: 200px;
  width: 20%;
  max-width: 600px;
  margin-left: 16px;
  margin-bottom: 16px;
  margin-right: 8px;
}

.tabSelectorsContainer {
  display: flex;
  justify-content: space-around;
  margin: 16px 0;
}

.tabSelector {
  cursor: pointer;
  padding: 8px 16px;
  border-radius: 8px;
  transition: scale 0.3s ease;
  opacity: 0.4;
}

.tabSelector>h3 {
  color: white;
  font-weight: bold;
}

.tabSelector:hover {
  background-color: var(--highlighted-panel-color);
  transition: scale 0.3s ease, background-color 0.3s ease, opacity 0.3s ease;
  opacity: 1.0;
}

.tabSelector:active {
  transition: scale 0.3s ease, opacity 0.3s ease;
  opacity: 1.0;
}

.selectedTab {
  background-color: var(--accent-color) !important;
  transition: transform;
  ;
  opacity: 1 !important;
}

.contentContainer {
  display: flex;
  flex-direction: column;
  flex: 1;
  overflow-y: auto;
}

.playlistsContainer {
  display: flex;
  flex-direction: column;
  flex: 1;
}

.createPlaylistButton {
  margin: 0 16px;
  padding: 8px;
  border-radius: 8px;
  padding: 8px 16px;
  width: fit-content;
  cursor: pointer;
  transition: background-color 0.3s ease;
}

.createPlaylistButton:hover {
  background-color: var(--highlighted-panel-color);
  transition: background-color 0.3s ease;
}
</style>
