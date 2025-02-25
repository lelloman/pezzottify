<template>
  <div class="playlistWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="playlistData" class="playlistData">
      <h1 class="playlistName">{{ playlistData.name }}</h1>
      <div class="commandsSection">
        <PlayIcon class="commandIcon" @click.stop="handleClickOnPlay" />
        <TrashIcon class="commandIcon" @click.stop="handleClickOnDelete" />
      </div>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>

  <Transition>
    <ConfirmationDialog v-if="deleteConfirmationDialogOpen" :isOpen="deleteConfirmationDialogOpen"
      :closeCallback="() => deleteConfirmationDialogOpen = false" :title="'Delete playlist'"
      :message="'Are you sure you want to delete <b>' + playlistData.name + '</b>?'"
      :positiveButtonCallback="handleDeletePlaylistConfirmation">

      <template #message>
        Are you sure you want to delete playlist <span style="font-weight: bold;">{{ playlistData.name }}</span>?
      </template>
    </ConfirmationDialog>
  </Transition>

</template>

<script setup>
import { ref, onMounted } from 'vue';
import axios from 'axios';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import TrashIcon from '../icons/TrashIcon.vue';
import ConfirmationDialog from '@/components/common/ConfirmationDialog.vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '@/store/user';

// Define playlistId prop
const props = defineProps({
  playlistId: {
    type: String,
    required: true,
  }
});

const router = useRouter();
const userStore = useUserStore();

const playlistData = ref(null);
const loading = ref(true);
const error = ref(null);

const deleteConfirmationDialogOpen = ref(false);

// Fetch playlist data
const fetchPlaylistData = async (id) => {
  try {
    const response = await axios.get(`/v1/user/playlist/${id}`);
    playlistData.value = response.data;
  } catch (err) {
    error.value = err.message;
  } finally {
    loading.value = false;
  }
};

const handleDeletePlaylistConfirmation = async () => {
  deleteConfirmationDialogOpen.value = false;
  userStore.deletePlaylist(props.playlistId, () => router.push('/'));
};

const handleClickOnPlay = () => {
  console.log('Play playlist:', props.playlistId);
};

const handleClickOnDelete = () => {
  console.log('Delete playlist:', props.playlistId);
  deleteConfirmationDialogOpen.value = true;
};

onMounted(() => {
  fetchPlaylistData(props.playlistId);
});
</script>

<style scoped>
.playlistData {
  display: flex;
  flex-direction: column;
  margin: 8px;
}

.playlistName {
  font-size: 34px;
}

.commandsSection {
  display: flex;
  flex-direction: row;
  margin-top: 16px;
  margin-left: 8px;
  margin-right: 8px;
  gap: 16px;
}

.commandIcon {
  scale: 1;
  fill: var(--accent-color);
  cursor: pointer;
  transition: scale 0.3s ease;
}

.commandIcon:hover {
  scale: 1.1;
  transition: scale 0.3s ease;
}

.commandIcon:active {
  scale: 0.9;
  transition: scale 0.3s ease;
}

.playlistConfirmationName {
  font-weight: bold;
  color: red;
}
</style>
