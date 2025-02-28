<template>
  <ContextMenu ref="contextMenu" :items="menuItems" />
</template>

<script setup>
import PlusIcon from '@/components/icons/PlusIcon.vue';
import ContextMenu from '@/components/common/contextmenu/ContextMenu.vue';
import { ref, markRaw } from 'vue';
import PlaylistPlusIcon from '@/components/icons/PlaylistPlusIcon.vue';
import { useUserStore } from '@/store/user';

const contextMenu = ref(null);
const userStore = useUserStore();

const track = ref(null);

const handleAddToQueueClick = (option) => {
  contextMenu.value.$emit('select', option);
};

const makeAddToPlaylistSubMenu = () => {
  return userStore.playlistsData.list.map(playlistId => (
    {
      name: userStore.playlistsData.by_id[playlistId].name,
      action: () => userStore.addTracksToPlaylist(playlistId, [track.value.id], () => { })
    }
  ));
}

const menuItems = ref([
  {
    icon: markRaw(PlusIcon),
    name: 'Add to playlist',
    subMenu: makeAddToPlaylistSubMenu
  },
  {
    icon: markRaw(PlaylistPlusIcon),
    name: 'Add to queue',
    action: () => handleAddToQueueClick('add')
  }
]);

const openMenu = (event, selectedTrack) => {
  track.value = selectedTrack;
  contextMenu.value.openMenu(event);
};

defineExpose({
  openMenu,
});
</script>

<style scoped>
@import '@/assets/icons.css';

.contextMenuItem {
  display: flex;
  flex-direction: row;
  padding: 8px;
  height: 50px;
  cursor: pointer;
  align-items: center;
  font-size: 14px;
  padding: 0 16px;
}

.contextMenuItem span {
  flex: 1;
}

.contextMenuItem:hover {
  background-color: #222;
}

.subMenu {
  z-index: 1001;
  position: fixed;
  width: 200px;
  border: 1px solid #ccc;
  background-color: #151515;
  z-index: 1001;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
