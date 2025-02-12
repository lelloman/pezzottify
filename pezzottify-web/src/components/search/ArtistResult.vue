<template>
    <div class="searchResultRow">
        <img :src="imageUrl" alt="Image" class="searchResultRoundImage" />
        <h3 class="title">{{ result.name }}</h3>
    </div>
</template>

<script setup>
import '@/assets/search.css'
import { computeImageUrl } from '@/utils';

const props = defineProps({
    result: {
        type: Object,
        required: true,
    }
});
const imageUrl = computeImageUrl(props.result.image_id);

</script>

<style scoped>
.title {
    margin: 0;
    font-size: 16px;
    font-weight: bold;
}
</style>
