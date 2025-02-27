<template>
  <div class="playlistWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="playlist" class="playlistData">
      <div class="nameRow">
        <h1 class="playlistNameLabel">
          {{ playlist.name }}
        </h1>

        <EditIcon class="editIcon scaleClickFeedback" @click.stop="handleEditButtonClick" />

      </div>
      <div class="commandsSection">
        <PlayIcon class="commandIcon scaleClickFeedback bigIcon" @click.stop="handleClickOnPlay" />
        <TrashIcon class="commandIcon scaleClickFeedback mediumIcon" @click.stop="handleClickOnDelete" />
      </div>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>

  <Transition>
    <ConfirmationDialog v-if="deleteConfirmationDialogOpen" :isOpen="deleteConfirmationDialogOpen"
      :closeCallback="() => deleteConfirmationDialogOpen = false" :title="'Delete playlist'"
      :positiveButtonCallback="handleDeletePlaylistConfirmation">

      <template #message>
        Are you sure you want to delete playlist <span style="font-weight: bold;">{{ playlist?.name }}</span>?
      </template>
    </ConfirmationDialog>
  </Transition>

  <Transition>
    <ConfirmationDialog v-if="isEditMode" :isOpen="isEditMode" :closeCallback="closeEditMode"
      :title="'Edit playlist name'" :negativeButtonText="'Cancel'" :positiveButtonText="'Save'"
      :positiveButtonCallback="handleChangeNameButtonClicked">

      <template #message>
        <input id="editPlaylistNameInput" />
      </template>

    </ConfirmationDialog>
  </Transition>
</template>

<script setup>
import { watch, ref, onMounted, computed } from 'vue';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import TrashIcon from '../icons/TrashIcon.vue';
import ConfirmationDialog from '@/components/common/ConfirmationDialog.vue';
import { useRoute, useRouter } from 'vue-router';
import { useUserStore } from '@/store/user';
import EditIcon from '@/components/icons/EditIcon.vue';

// Define playlistId prop
const props = defineProps({
  playlistId: {
    type: String,
    required: true,
  }
});

const router = useRouter();
const route = useRoute();
const userStore = useUserStore();

const loading = ref(true);
const error = ref(null);

// Get reactive reference to the playlist
const playlist = computed(() => {
  const playlistRef = userStore.getPlaylistRef(props.playlistId);
  return playlistRef?.value; // Unwrap the computed ref
});

const deleteConfirmationDialogOpen = ref(false);
const isEditMode = ref(false);

const handleEditButtonClick = () => {
  router.push({ query: { edit: !isEditMode.value } });
}

// Load playlist data
const loadData = async () => {
  if (!props.playlistId) return;

  loading.value = true;
  try {
    await userStore.loadPlaylistData(props.playlistId);
    error.value = null;
  } catch (e) {
    error.value = "Failed to load playlist data";
    console.error(e);
  } finally {
    loading.value = false;
  }
};

const handleChangeNameButtonClicked = async () => {
  const newName = document.getElementById("editPlaylistNameInput").value;
  closeEditMode();
  userStore.updatePlaylistName(props.playlistId, newName);
};

const closeEditMode = () => {
  router.push({});
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

watch(
  route,
  (newRoute) => {
    isEditMode.value = newRoute.query.edit;
    if (isEditMode.value && playlist.value) {
      setTimeout(() => {
        document.getElementById("editPlaylistNameInput").value = playlist.value.name;
        document.getElementById("editPlaylistNameInput").focus();
      }, 100);
    }
  },
  { immediate: true }
);

// Watch for playlist ID changes to load data
watch(
  () => props.playlistId,
  (newId) => {
    if (newId) {
      loadData();
    }
  },
  { immediate: true }
);

onMounted(() => {
  loadData();
});
</script>

<style scoped>
@import "@/assets/icons.css";

.playlistData {
  display: flex;
  flex-direction: column;
  margin: 8px;
}

.nameRow {
  display: flex;
  flex-direction: row;
  align-items: center;
}

.playlistNameLabel {
  font-size: 34px;
  flex: 1;
}

.editIcon {
  fill: white;
  height: 32px;
  width: 32px;
}

.commandsSection {
  display: flex;
  flex-direction: row;
  margin-top: 16px;
  margin-left: 8px;
  margin-right: 8px;
  gap: 16px;
  align-items: center;
}

.commandIcon {
  fill: var(--accent-color);
}

.playlistConfirmationName {
  font-weight: bold;
  color: red;
}

#playlistNameInput {
  font-size: 34px;
  flex: 1;
  background-color: transparent;
  border: none;
  color: white;
  font-weight: bold;
  font-size: 34px;
  padding: 0;
  margin: 0;
  outline: none;
  border-bottom: 2px solid white;
  margin-right: 16px;
}
</style>
