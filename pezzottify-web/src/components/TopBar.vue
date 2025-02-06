<template>
    <header class="bg-gray-700 p-4 flex items-center">
        <input type="text" placeholder="Search..." class="flex-1 bg-gray-600 text-white rounded p-2 focus:outline-none"
            @input="onInput" />
    </header>
</template>

<script setup>
import { ref } from 'vue';
import { debounce } from 'lodash-es'; // Lightweight debounce

const emit = defineEmits(['search']);
const inputValue = ref('');

const debounceEmit = debounce((value) => {
    emit('search', value);
}, 300); // 300ms debounce

function onInput(event) {
    inputValue.value = event.target.value;
    debounceEmit(inputValue.value);
}
</script>