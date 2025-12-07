<template>
  <div class="analyticsDashboard">
    <h2 class="sectionTitle">Analytics Dashboard</h2>

    <!-- Date Range Picker -->
    <div class="dateRangeSection">
      <div class="dateInputs">
        <label class="dateLabel">
          From:
          <input
            type="date"
            v-model="startDate"
            class="dateInput"
          />
        </label>
        <label class="dateLabel">
          To:
          <input
            type="date"
            v-model="endDate"
            class="dateInput"
          />
        </label>
        <button class="refreshButton" @click="loadData" :disabled="isLoading">
          {{ isLoading ? 'Loading...' : 'Refresh' }}
        </button>
      </div>
    </div>

    <!-- Online Users -->
    <div class="onlineUsersCard">
      <div class="onlineUsersInfo">
        <span class="onlineCount">{{ onlineUsers?.count ?? '—' }}</span>
        <span class="onlineLabel">{{ onlineUsers?.count === 1 ? 'user online' : 'users online' }}</span>
      </div>
      <div v-if="onlineUsers?.handles?.length > 0" class="onlineHandles">
        <span v-for="handle in onlineUsers.handles" :key="handle" class="userBadge">
          {{ handle }}
        </span>
        <span v-if="onlineUsers.count > 3" class="moreUsers">
          +{{ onlineUsers.count - 3 }} more
        </span>
      </div>
    </div>

    <div v-if="loadError" class="errorMessage">
      {{ loadError }}
    </div>

    <!-- Daily Listening Stats -->
    <div class="chartSection">
      <h3 class="chartTitle">Daily Listening</h3>
      <div class="chartContainer">
        <Line v-if="dailyChartData" :data="dailyChartData" :options="lineChartOptions" />
        <div v-else class="noData">No listening data available for this period.</div>
      </div>
    </div>

    <!-- Daily Stats Table -->
    <div v-if="dailyStats.length > 0" class="tableSection">
      <h4 class="tableTitle">Daily Breakdown</h4>
      <div class="tableWrapper">
        <table class="dataTable">
          <thead>
            <tr>
              <th>Date</th>
              <th>Plays</th>
              <th>Completed</th>
              <th>Duration</th>
              <th>Users</th>
              <th>Tracks</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="day in dailyStats" :key="day.date">
              <td>{{ formatDate(day.date) }}</td>
              <td>{{ day.total_plays }}</td>
              <td>{{ day.completed_plays }}</td>
              <td>{{ formatDuration(day.total_duration_seconds) }}</td>
              <td>{{ day.unique_users }}</td>
              <td>{{ day.unique_tracks }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Top Tracks -->
    <div class="chartSection">
      <h3 class="chartTitle">Top Tracks</h3>
      <div class="chartContainer barChartContainer">
        <Bar v-if="topTracksChartData" :data="topTracksChartData" :options="barChartOptions" />
        <div v-else class="noData">No track data available for this period.</div>
      </div>
    </div>

    <!-- Top Tracks Table -->
    <div v-if="topTracks.length > 0" class="tableSection">
      <h4 class="tableTitle">Top Tracks Breakdown</h4>
      <div class="tableWrapper">
        <table class="dataTable">
          <thead>
            <tr>
              <th>#</th>
              <th>Track</th>
              <th>Artist</th>
              <th>Plays</th>
              <th>Completed</th>
              <th>Duration</th>
              <th>Listeners</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(track, index) in topTracksWithInfo" :key="track.track_id">
              <td>{{ index + 1 }}</td>
              <td class="trackName">{{ track.name || track.track_id }}</td>
              <td class="artistName">{{ track.artist || '—' }}</td>
              <td>{{ track.play_count }}</td>
              <td>{{ track.completed_count }}</td>
              <td>{{ formatDuration(track.total_duration_seconds) }}</td>
              <td>{{ track.unique_listeners }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { Line, Bar } from 'vue-chartjs';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from 'chart.js';
import { useRemoteStore } from '@/store/remote';

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  BarElement,
  Title,
  Tooltip,
  Legend,
  Filler
);

const remoteStore = useRemoteStore();

// Date range - default to last 30 days
const today = new Date();
const thirtyDaysAgo = new Date(today);
thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);

const formatDateForInput = (date) => date.toISOString().split('T')[0];
const formatDateForApi = (dateStr) => dateStr.replace(/-/g, '');

const startDate = ref(formatDateForInput(thirtyDaysAgo));
const endDate = ref(formatDateForInput(today));

const isLoading = ref(false);
const loadError = ref(null);

const dailyStats = ref([]);
const topTracks = ref([]);
const trackInfoMap = ref({});
const onlineUsers = ref(null);

// Fetch track info for top tracks
const fetchTrackInfo = async () => {
  for (const track of topTracks.value) {
    if (!trackInfoMap.value[track.track_id]) {
      const trackInfo = await remoteStore.fetchResolvedTrack(track.track_id);
      if (trackInfo) {
        trackInfoMap.value = {
          ...trackInfoMap.value,
          [track.track_id]: trackInfo,
        };
      }
    }
  }
};

// Watch topTracks and fetch info when it changes
watch(topTracks, () => {
  fetchTrackInfo();
});

// Computed that combines track stats with track info
const topTracksWithInfo = computed(() => {
  return topTracks.value.map(track => {
    const info = trackInfoMap.value[track.track_id];
    return {
      ...track,
      name: info?.track?.name || null,
      artist: info?.artists?.[0]?.artist?.name || null,
    };
  });
});

const loadData = async () => {
  isLoading.value = true;
  loadError.value = null;

  const start = formatDateForApi(startDate.value);
  const end = formatDateForApi(endDate.value);

  const [dailyResult, topTracksResult, onlineResult] = await Promise.all([
    remoteStore.fetchDailyListening(start, end),
    remoteStore.fetchTopTracks(start, end, 20),
    remoteStore.fetchOnlineUsers(),
  ]);

  if (dailyResult === null && topTracksResult === null) {
    loadError.value = 'Failed to load analytics data.';
  } else {
    dailyStats.value = dailyResult || [];
    topTracks.value = topTracksResult || [];
  }

  onlineUsers.value = onlineResult;

  isLoading.value = false;
};

// Format date from YYYYMMDD to readable format
const formatDate = (dateNum) => {
  const str = String(dateNum);
  const year = str.slice(0, 4);
  const month = str.slice(4, 6);
  const day = str.slice(6, 8);
  return `${year}-${month}-${day}`;
};

// Format seconds to human readable duration
const formatDuration = (seconds) => {
  if (!seconds) return '0m';
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  if (hours > 0) {
    return `${hours}h ${mins}m`;
  }
  return `${mins}m`;
};

// Chart data for daily listening
const dailyChartData = computed(() => {
  if (dailyStats.value.length === 0) return null;

  const sortedStats = [...dailyStats.value].sort((a, b) => a.date - b.date);

  return {
    labels: sortedStats.map(d => formatDate(d.date)),
    datasets: [
      {
        label: 'Total Plays',
        data: sortedStats.map(d => d.total_plays),
        borderColor: '#1db954',
        backgroundColor: 'rgba(29, 185, 84, 0.1)',
        fill: true,
        tension: 0.3,
      },
      {
        label: 'Completed Plays',
        data: sortedStats.map(d => d.completed_plays),
        borderColor: '#1ed760',
        backgroundColor: 'rgba(30, 215, 96, 0.1)',
        fill: true,
        tension: 0.3,
      },
    ],
  };
});

// Chart data for top tracks
const topTracksChartData = computed(() => {
  if (topTracksWithInfo.value.length === 0) return null;

  const top10 = topTracksWithInfo.value.slice(0, 10);

  return {
    labels: top10.map((t, i) => {
      const name = t.name || `Track ${i + 1}`;
      // Truncate long names for chart readability
      return name.length > 20 ? name.slice(0, 17) + '...' : name;
    }),
    datasets: [
      {
        label: 'Play Count',
        data: top10.map(t => t.play_count),
        backgroundColor: 'rgba(29, 185, 84, 0.8)',
        borderColor: '#1db954',
        borderWidth: 1,
      },
    ],
  };
});

const lineChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      position: 'top',
      labels: {
        color: '#b3b3b3',
      },
    },
  },
  scales: {
    x: {
      ticks: { color: '#b3b3b3' },
      grid: { color: 'rgba(255, 255, 255, 0.1)' },
    },
    y: {
      ticks: { color: '#b3b3b3' },
      grid: { color: 'rgba(255, 255, 255, 0.1)' },
      beginAtZero: true,
    },
  },
};

const barChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      display: false,
    },
  },
  scales: {
    x: {
      ticks: { color: '#b3b3b3' },
      grid: { color: 'rgba(255, 255, 255, 0.1)' },
    },
    y: {
      ticks: { color: '#b3b3b3' },
      grid: { color: 'rgba(255, 255, 255, 0.1)' },
      beginAtZero: true,
    },
  },
};

// Refresh online users only (for periodic polling)
const refreshOnlineUsers = async () => {
  const result = await remoteStore.fetchOnlineUsers();
  if (result !== null) {
    onlineUsers.value = result;
  }
};

// Polling interval for online users (30 seconds)
const ONLINE_USERS_POLL_INTERVAL = 30000;
let onlineUsersInterval = null;

onMounted(() => {
  loadData();
  onlineUsersInterval = setInterval(refreshOnlineUsers, ONLINE_USERS_POLL_INTERVAL);
});

onUnmounted(() => {
  if (onlineUsersInterval) {
    clearInterval(onlineUsersInterval);
    onlineUsersInterval = null;
  }
});
</script>

<style scoped>
.analyticsDashboard {
  width: 100%;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-6) 0;
}

.dateRangeSection {
  margin-bottom: var(--spacing-4);
}

.onlineUsersCard {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  margin-bottom: var(--spacing-6);
}

.onlineUsersInfo {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-2);
}

.onlineCount {
  font-size: var(--text-3xl);
  font-weight: var(--font-bold);
  color: var(--spotify-green);
}

.onlineLabel {
  font-size: var(--text-base);
  color: var(--text-subdued);
}

.onlineHandles {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--spacing-2);
}

.userBadge {
  padding: var(--spacing-1) var(--spacing-3);
  background-color: rgba(29, 185, 84, 0.15);
  border: 1px solid rgba(29, 185, 84, 0.3);
  border-radius: var(--radius-full);
  font-size: var(--text-sm);
  color: var(--spotify-green);
}

.moreUsers {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.dateInputs {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-4);
  align-items: flex-end;
}

.dateLabel {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.dateInput {
  padding: var(--spacing-2) var(--spacing-3);
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-base);
}

.refreshButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.refreshButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.refreshButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.errorMessage {
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-4);
}

.chartSection {
  margin-bottom: var(--spacing-6);
}

.chartTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-3) 0;
}

.chartContainer {
  height: 300px;
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
}

.barChartContainer {
  height: 250px;
}

.noData {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-subdued);
}

.tableSection {
  margin-bottom: var(--spacing-6);
}

.tableTitle {
  font-size: var(--text-base);
  font-weight: var(--font-medium);
  color: var(--text-subdued);
  margin: 0 0 var(--spacing-2) 0;
}

.tableWrapper {
  overflow-x: auto;
}

.dataTable {
  width: 100%;
  border-collapse: collapse;
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.dataTable th,
.dataTable td {
  padding: var(--spacing-3) var(--spacing-4);
  text-align: left;
  border-bottom: 1px solid var(--border-subdued);
}

.dataTable th {
  background-color: var(--bg-highlight);
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.dataTable td {
  color: var(--text-base);
  font-size: var(--text-sm);
}

.dataTable tr:last-child td {
  border-bottom: none;
}

.dataTable tr:hover td {
  background-color: var(--bg-highlight);
}

.trackId {
  font-family: monospace;
  font-size: var(--text-xs);
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 768px) {
  .dateInputs {
    flex-direction: column;
    align-items: stretch;
  }

  .dateLabel {
    width: 100%;
  }

  .refreshButton {
    width: 100%;
  }

  .chartContainer {
    height: 250px;
  }

  .barChartContainer {
    height: 200px;
  }
}
</style>
