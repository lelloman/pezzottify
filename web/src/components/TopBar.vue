<template>
  <header>
    <div class="searchInputContainer">
      <div class="searchBar">
        <input class="searchInput" type="text" placeholder="Search..." @input="onInput" inputmode="search"
          v-model="localQuery" />
        <button v-if="localQuery" id="clearQueryButton" type="submit" name="clearQueryButton" @click="clearQuery()">
          <CrossIcon class="scaleClickFeedback crossIcon" />
        </button>
      </div>
    </div>
  </header>
</template>

<script setup>
import { ref, watch } from 'vue';
import { debounce } from 'lodash-es'; // Lightweight debounce
import { useRouter, useRoute } from 'vue-router';
import CrossIcon from './icons/CrossIcon.vue';

const emit = defineEmits(['search']);
const inputValue = ref('');
const router = useRouter();
const route = useRoute();

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
    console.log("TopBar changing search query, current path query: " + route.query);
    router.push({ path: `/search/${encodeURIComponent(value.trim())}`, query: route.query });
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
  background: var(--bg-highlight);
  color: var(--text-base);
  outline: none;
  border: 1px solid var(--border-default);
  border-radius: 1.625rem;
  padding: 0 3.5rem 0 1.5rem;
  font-size: 1rem;
  transition: border-color var(--transition-fast), background-color var(--transition-fast);
}

.searchInput::placeholder {
  color: var(--text-subdued);
}

.searchInput:focus {
  border-color: var(--spotify-green);
  background: var(--bg-elevated);
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

.crossIcon {
  width: 24px;
  height: 24px;
  stroke: #666;
}
</style>
