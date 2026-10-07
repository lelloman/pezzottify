<template>
  <div class="analyticsDashboard">
    <header class="pageHeader">
      <h2 class="sectionTitle">Analytics</h2>
      <p>Explore listening activity, popular tracks and who’s online.</p>
    </header>

    <!-- Date Range Picker -->
    <form class="dateRangeSection" @submit.prevent="loadData">
      <div class="dateInputs">
        <label class="dateLabel">
          From
          <input
            type="date"
            v-model="startDate"
            :max="endDate"
            required
            class="dateInput"
          />
        </label>
        <label class="dateLabel">
          To
          <input
            type="date"
            v-model="endDate"
            :min="startDate"
            required
            class="dateInput"
          />
        </label>
        <button class="refreshButton" type="submit" :disabled="isLoading">
          {{ isLoading ? "Loading…" : "Refresh" }}
        </button>
      </div>
    </form>

    <!-- Online Users -->
    <div class="onlineUsersCard">
      <div class="onlineUsersInfo">
        <span class="onlineCount">{{ onlineUsers?.count ?? "—" }}</span>
        <span class="onlineLabel">{{
          onlineUsers?.count === 1 ? "user online" : "users online"
        }}</span>
      </div>
      <div v-if="onlineUsers?.handles?.length > 0" class="onlineHandles">
        <span
          v-for="handle in onlineUsers.handles.slice(0, 3)"
          :key="handle"
          class="userBadge"
        >
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

    <!-- Daily listening Stats -->
    <div class="chartSection">
      <h3 class="chartTitle">Daily listening</h3>
      <div class="chartContainer">
        <Line
          v-if="dailyChartData"
          :data="dailyChartData"
          :options="lineChartOptions"
          aria-label="Daily total and completed plays; values listed in the table below"
          role="img"
        />
        <div v-else class="noData">
          No listening data available for this period.
        </div>
      </div>
    </div>

    <!-- Daily Stats Table -->
    <div v-if="dailyStats.length > 0" class="tableSection">
      <h4 class="tableTitle">Daily breakdown</h4>
      <div
        class="tableWrapper"
        tabindex="0"
        role="region"
        aria-label="Daily listening breakdown"
      >
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

    <!-- Top tracks -->
    <div class="chartSection">
      <h3 class="chartTitle">Top tracks</h3>
      <div class="chartContainer barChartContainer">
        <Bar
          v-if="topTracksChartData"
          :data="topTracksChartData"
          :options="barChartOptions"
          aria-label="Top ten tracks by plays; values listed in the table below"
          role="img"
        />
        <div v-else class="noData">
          No track data available for this period.
        </div>
      </div>
    </div>

    <!-- Top tracks Table -->
    <div v-if="topTracks.length > 0" class="tableSection">
      <h4 class="tableTitle">Top tracks breakdown</h4>
      <div
        class="tableWrapper"
        tabindex="0"
        role="region"
        aria-label="Top tracks breakdown"
      >
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
            <tr
              v-for="(track, index) in topTracksWithInfo"
              :key="track.track_id"
            >
              <td>{{ index + 1 }}</td>
              <td class="trackName">{{ track.name || track.track_id }}</td>
              <td class="artistName">{{ track.artist || "—" }}</td>
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
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { Line, Bar } from "vue-chartjs";
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
} from "chart.js";
import { useRemoteStore } from "@/store/remote";

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
  Filler,
);

const remoteStore = useRemoteStore();

// Date range - default to last 30 days
const today = new Date();
const thirtyDaysAgo = new Date(today);
thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);

const formatDateForInput = (date) => date.toISOString().split("T")[0];
const formatDateForApi = (dateStr) => dateStr.replace(/-/g, "");

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
  return topTracks.value.map((track) => {
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
    loadError.value = "Failed to load analytics data.";
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
  if (!seconds) return "0m";
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
    labels: sortedStats.map((d) => formatDate(d.date)),
    datasets: [
      {
        label: "Total Plays",
        data: sortedStats.map((d) => d.total_plays),
        borderColor: "#1ed760",
        backgroundColor: "rgba(30, 215, 96, 0.08)",
        borderWidth: 2,
        pointRadius: 2,
        fill: true,
        tension: 0.3,
      },
      {
        label: "Completed Plays",
        data: sortedStats.map((d) => d.completed_plays),
        borderColor: "#b3b3b3",
        backgroundColor: "#b3b3b3",
        borderDash: [5, 4],
        borderWidth: 2,
        pointRadius: 2,
        fill: false,
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
      return name.length > 20 ? name.slice(0, 17) + "..." : name;
    }),
    datasets: [
      {
        label: "Play Count",
        data: top10.map((t) => t.play_count),
        backgroundColor: "#1ed760",
        borderRadius: 4,
        maxBarThickness: 24,
        borderColor: "#1ed760",
        borderWidth: 1,
      },
    ],
  };
});

const chartFont = {
  family: "Figtree, Helvetica Neue, Arial, sans-serif",
  size: 12,
};
const lineChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      position: "top",
      labels: {
        color: "#b3b3b3",
        font: chartFont,
        usePointStyle: true,
        boxWidth: 8,
        padding: 20,
      },
    },
  },
  scales: {
    x: {
      ticks: {
        color: "#b3b3b3",
        font: chartFont,
        maxRotation: 0,
        maxTicksLimit: 6,
      },
      grid: { color: "rgba(255, 255, 255, 0.06)" },
    },
    y: {
      ticks: {
        color: "#b3b3b3",
        font: chartFont,
        maxRotation: 0,
        maxTicksLimit: 6,
      },
      grid: { color: "rgba(255, 255, 255, 0.06)" },
      beginAtZero: true,
    },
  },
};

const barChartOptions = {
  indexAxis: "y",
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      display: false,
    },
  },
  scales: {
    x: {
      beginAtZero: true,
      ticks: {
        color: "#b3b3b3",
        font: chartFont,
        maxRotation: 0,
        maxTicksLimit: 6,
      },
      grid: { color: "rgba(255, 255, 255, 0.06)" },
    },
    y: {
      ticks: {
        color: "#b3b3b3",
        font: chartFont,
        maxRotation: 0,
        autoSkip: false,
      },
      grid: { color: "rgba(255, 255, 255, 0.06)" },
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
  onlineUsersInterval = setInterval(
    refreshOnlineUsers,
    ONLINE_USERS_POLL_INTERVAL,
  );
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
  min-width: 0;
  color: var(--text-base);
  color-scheme: dark;
}
.pageHeader {
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.04em;
}
.pageHeader p {
  margin: 8px 0 0;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.5;
}
.dateRangeSection {
  margin-bottom: 24px;
}
.dateInputs {
  display: flex;
  align-items: flex-end;
  flex-wrap: wrap;
  gap: 16px;
}
.dateLabel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
}
.dateInput {
  box-sizing: border-box;
  min-width: 0;
  min-height: 44px;
  padding: 10px 12px;
  border: 1px solid #727272;
  border-radius: 4px;
  background: #242424;
  color: #fff;
  font: inherit;
  font-size: 14px;
}
.dateInput:focus {
  outline: 2px solid white;
  outline-offset: -2px;
}
.refreshButton {
  min-height: 44px;
  padding: 10px 24px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: white;
  font: inherit;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}
.refreshButton:hover:not(:disabled) {
  border-color: white;
  background: #242424;
}
.refreshButton:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.refreshButton:focus-visible,
.tableWrapper:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
.onlineUsersCard {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 20px;
  padding: 20px 24px;
  border-radius: 8px;
  background: #181818;
  margin-bottom: 28px;
}
.onlineUsersInfo {
  display: flex;
  align-items: baseline;
  gap: 10px;
}
.onlineUsersInfo::before {
  content: "";
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #1ed760;
  align-self: center;
}
.onlineCount {
  font-size: 28px;
  font-weight: 700;
  letter-spacing: -0.03em;
}
.onlineLabel,
.moreUsers {
  font-size: 14px;
  color: var(--text-subdued);
}
.onlineHandles {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}
.userBadge {
  padding: 6px 12px;
  border-radius: 999px;
  background: #2a2a2a;
  color: #b3b3b3;
  font-size: 13px;
  overflow-wrap: anywhere;
}
.errorMessage {
  padding: 16px;
  border-radius: 8px;
  background: #281a1d;
  color: #f3727f;
  font-size: 14px;
  margin-bottom: 24px;
}
.chartSection {
  margin-bottom: 20px;
  padding: 24px;
  border-radius: 8px;
  background: #181818;
}
.chartTitle {
  margin: 0 0 20px;
  font-size: 20px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.chartContainer {
  position: relative;
  height: 300px;
  min-width: 0;
}
.barChartContainer {
  height: 360px;
}
.noData {
  display: flex;
  height: 100%;
  align-items: center;
  justify-content: center;
  color: var(--text-subdued);
  font-size: 14px;
  text-align: center;
}
.tableSection {
  margin-bottom: 32px;
}
.tableTitle {
  margin: 0 0 12px;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-subdued);
}
.tableWrapper {
  overflow-x: auto;
  border-radius: 8px;
  background: #181818;
}
.dataTable {
  width: 100%;
  min-width: 680px;
  border-collapse: collapse;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}
.dataTable th,
.dataTable td {
  padding: 14px 16px;
  text-align: left;
  border-bottom: 1px solid #282828;
}
.dataTable th {
  color: var(--text-subdued);
  font-size: 12px;
  font-weight: 500;
}
.dataTable td {
  color: #fff;
}
.dataTable .artistName {
  color: var(--text-subdued);
}
.dataTable tr:last-child td {
  border-bottom: 0;
}
.dataTable tbody tr:hover {
  background: #242424;
}
@media (max-width: 600px) {
  .dateLabel {
    flex: 1 1 140px;
  }
  .onlineUsersCard,
  .chartSection {
    padding: 16px;
  }
  .chartContainer {
    height: 260px;
  }
  .barChartContainer {
    height: 360px;
  }
}
</style>
