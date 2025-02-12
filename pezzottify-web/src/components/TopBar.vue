<template>
  <header class="searchInputContainer">
    <input type="text" placeholder="Search..." @input="onInput" v-model="localQuery" />
  </header>
</template>

<script setup>
import { ref, watch } from 'vue';
import { debounce } from 'lodash-es'; // Lightweight debounce
import { useRouter } from 'vue-router';

const emit = defineEmits(['search']);
const inputValue = ref('');
const router = useRouter();

const props = defineProps({
  initialQuery: {
    type: String,
    default: '',
  },
});

const localQuery = ref(props.initialQuery);
watch(
  () => props.initialQuery,
  (newQuery) => {
    localQuery.value = newQuery;
  }
);

const debounceEmit = debounce((value) => {
  const trimmed = value.trim();
  if (trimmed.length > 0) {
    router.push({ path: `/search/${encodeURIComponent(value.trim())}` });
  } else {
    router.push({ path: "/" });
  }
  emit('search', value);
}, 300); // 300ms debounce

function onInput(event) {
  inputValue.value = event.target.value;
  debounceEmit(inputValue.value);
}

</script>

<style scoped>
.searchInputContainer {
  padding: 16px;
}
</style>
