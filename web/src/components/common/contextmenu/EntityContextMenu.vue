<template>
  <ContextMenu ref="contextMenu" :items="menuItems" />
  <RadioBuilderModal
    v-if="entity"
    :isOpen="showRadioBuilder"
    :seedEntityType="entity.entity_type"
    :seedEntityId="entity.entity_id"
    @close="showRadioBuilder = false"
  />
  <DestinationStepsPrompt
    :isOpen="showDestinationPrompt"
    :reference="entity"
    @close="showDestinationPrompt = false"
  />
</template>

<script setup>
// Right-click menu for albums and artists (tracks use TrackContextMenu).
import { markRaw, ref } from "vue";
import ContextMenu from "@/components/common/contextmenu/ContextMenu.vue";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";
import DestinationStepsPrompt from "@/components/common/DestinationStepsPrompt.vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
import SteeringWheelIcon from "@/components/icons/SteeringWheelIcon.vue";
import { usePlaybackStore } from "@/store/playback";

const playback = usePlaybackStore();

const contextMenu = ref(null);
// { entity_type: "album" | "artist", entity_id, label? }
const entity = ref(null);
const showRadioBuilder = ref(false);
const showDestinationPrompt = ref(false);

const menuItems = ref([
  {
    icon: markRaw(RadioIcon),
    name: "Listen to radio",
    action: () => {
      if (entity.value) {
        playback.setRadioFromItem(
          entity.value.entity_type,
          entity.value.entity_id,
        );
      }
    },
  },
  {
    icon: markRaw(RadioIcon),
    name: "Customize radio",
    action: () => {
      if (entity.value) showRadioBuilder.value = true;
    },
  },
  {
    icon: markRaw(SteeringWheelIcon),
    name: "Set as playback destination",
    action: () => {
      if (entity.value) showDestinationPrompt.value = true;
    },
  },
]);

const openMenu = (event, entityType, entityId, label = null) => {
  entity.value = {
    entity_type: entityType,
    entity_id: entityId,
    ...(label ? { label } : {}),
  };
  contextMenu.value.openMenu(event);
};

defineExpose({
  openMenu,
});
</script>
