<template>
    <div class="searchResultRow">
        <img :src="imageUrl" alt="Image" class="square-image" />
        <div class="column">
            <h3 class="title">{{ result.name }}</h3>
            <p class="subtitle">{{ result.artists_names.join(", ") }}</p>
        </div>
    </div>
</template>

<script setup>
import '@/assets/search.css'
import { computeImageUrl } from '@/utils.js';

const props = defineProps({
    result: {
        type: Object,
        required: true,
    }
});

const imageUrl = computeImageUrl(props.result.image_id);
</script>

<style scoped>
.square-image {
    width: 50px;
    height: 50px;
    margin-right: 10px;
}

.column {
    display: flex;
    flex-direction: column;
}

.title {
    margin: 0;
    font-size: 16px;
    font-weight: bold;
}

.subtitle {
    margin: 0;
    font-size: 14px;
    color: #666;
}
</style>
