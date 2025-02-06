<template>
    <aside class="sidebar">
        <nav>
            <div v-for="item in items" :key="item.id" class="mb-4">
                <h2 class="text-lg font-semibold" v-if="item.type === 'section'">{{ item.name }}</h2>
                <ul v-if="item.type === 'section'">
                    <li v-for="subItem in item.items" :key="subItem" class="pl-4 cursor-pointer hover:text-gray-400"
                        @click="$emit('select-item', subItem)">{{ subItem }}</li>
                </ul>
                <div v-else class="cursor-pointer hover:text-gray-400" @click="$emit('select-item', item.name)">{{
                    item.name }}</div>
            </div>
        </nav>
    </aside>
</template>

<script setup>
import { defineProps } from 'vue';
const props = defineProps({ items: Array });
</script>

<style scoped>
.sidebar {
    min-width: 200px;
    width: 20%;
    max-width: 600px;
    background-color: #1f2937;
    color: white;
    height: 100%;
    padding: 1rem;
    box-sizing: border-box;
}
</style>