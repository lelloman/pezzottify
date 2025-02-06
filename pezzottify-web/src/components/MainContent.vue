<template>
    <main class="flex-1 p-6 overflow-auto">
        <div v-if="searchQuery" class="text-lg">Showing results for "{{ searchQuery }}"</div>
        <div v-else>
            <h1 class="text-2xl font-bold mb-4">Welcome to Home</h1>
            <p>This is your home content.</p>
        </div>
    </main>
</template>

<script setup>
import { defineProps } from 'vue';
const props = defineProps({ searchQuery: String });
</script>