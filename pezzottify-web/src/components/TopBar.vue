<template>
    <header class="bg-gray-700 p-4 flex items-center">
        <input type="text" placeholder="Search..." class="flex-1 bg-gray-600 text-white rounded p-2 focus:outline-none"
            @input="onInput" />
    </header>
</template>

<script setup>
import { defineEmits } from 'vue';

const emit = defineEmits(['search']);

function onInput(event) {
    emit('search', event.target.value);
}
</script>