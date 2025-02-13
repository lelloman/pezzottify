<template>
  <header>
    <div class="searchInputContainer">
      <div class="searchBar">
        <input class="searchInput" type="text" placeholder="Search..." @input="onInput" inputmode="search"
          v-model="localQuery" />
        <button v-if="localQuery" id="clearQueryButton" type="submit" name="clearQueryButton" @click="clearQuery()">
          <svg width="24px" height="24px" fill="none" style="width:24px;height:24px" viewBox="0 0 24 24">
            <path stroke="#666" fill="#666666" d="M16 8L7.99997 16M7.99999 8L16 16" />
          </svg>
        </button>
      </div>
    </div>
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

function clearQuery() {
  router.push("/")
}

</script>

<style scoped>
.searchInputContainer {
  width: 100%;
  max-width: 31.25rem;
  margin: 1rem auto;
}

.searchBar {
  width: 100%;
  display: flex;
  flex-direction: row;
  align-items: center;
}

.searchInput {
  width: 100%;
  height: 2.8rem;
  background: #f5f5f5;
  outline: none;
  border: none;
  border-radius: 1.625rem;
  padding: 0 3.5rem 0 1.5rem;
  font-size: 1rem;
}

#clearQueryButton {
  width: 3.5rem;
  height: 2.8rem;
  margin-left: -3.5rem;
  background: none;
  border: none;
  outline: none;
}

#clearQueryButton:hover {
  cursor: pointer;
}
</style>
