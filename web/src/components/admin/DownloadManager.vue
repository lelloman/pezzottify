<template>
  <div class="downloadManager">
    <header class="pageHeader">
      <h2 class="sectionTitle">Downloads</h2>
      <p>Monitor the queue, resolve failures and review download activity.</p>
    </header>

    <!-- Action Buttons -->
    <div class="actionButtons">
      <button class="actionButton" @click="openDownloadModal('album')">
        Download Album
      </button>
      <button class="refreshButton" @click="loadData" :disabled="isLoading">
        {{ isLoading ? "Loading..." : "Refresh" }}
      </button>
    </div>

    <!-- Stats Summary -->
    <div class="statsSummary">
      <span class="statItem">
        <strong>{{ stats?.queue?.pending ?? 0 }}</strong> pending
      </span>
      <span class="statItem">
        <strong>{{ stats?.queue?.in_progress ?? 0 }}</strong> in progress
      </span>
      <span class="statItem">
        <strong>{{ stats?.queue?.retry_waiting ?? 0 }}</strong> retrying
      </span>
      <span class="statItem success">
        <strong>{{ stats?.queue?.completed_today ?? 0 }}</strong> completed
        today
      </span>
      <span class="statItem danger">
        <strong>{{ stats?.queue?.failed_today ?? 0 }}</strong> failed today
      </span>
    </div>

    <!-- Tab Navigation -->
    <div class="tabNav">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="tabButton"
        :class="{ active: activeTab === tab.id }"
        :aria-pressed="activeTab === tab.id"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
        <span v-if="tab.count !== undefined" class="tabCount">{{
          tab.count
        }}</span>
      </button>
    </div>

    <div v-if="loadError" class="errorMessage">
      {{ loadError }}
    </div>

    <!-- Proxy Downloads Tab -->
    <div v-if="activeTab === 'proxy'" class="tabContent">
      <div v-if="!proxyStatus?.enabled" class="emptyState">
        Track proxy downloading is not enabled on this server.
      </div>
      <template v-else>
        <div class="proxySummary">
          <span class="statItem">
            <strong>{{ proxyStatus.active.length }}</strong> active
          </span>
          <span class="statItem">
            <strong
              >{{ proxyStatus.foreground_active }}/{{
                proxyStatus.foreground_limit
              }}</strong
            >
            foreground slots
          </span>
          <span class="statItem">
            <strong
              >{{ proxyStatus.prefetch_active }}/{{
                proxyStatus.prefetch_limit
              }}</strong
            >
            prefetch slots
          </span>
          <span class="statItem">
            <strong>{{ formatBytes(proxyStatus.memory_used_bytes) }}</strong>
            / {{ formatBytes(proxyStatus.memory_limit_bytes) }} memory
          </span>
        </div>

        <h3 class="proxySectionTitle">Active</h3>
        <div v-if="proxyStatus.active.length === 0" class="emptyState compact">
          No tracks are being materialized right now.
        </div>
        <div v-else class="queueList">
          <div
            v-for="job in proxyStatus.active"
            :key="job.track_id"
            class="queueItem proxyJob"
          >
            <div class="queueItemHeader">
              <div class="queueItemMain">
                <span class="queueItemType">{{ job.priority }}</span>
                <span
                  class="queueItemName clickable"
                  role="link"
                  tabindex="0"
                  @keydown.enter="goToProxyTrack(job)"
                  @click="goToProxyTrack(job)"
                >
                  {{ job.track_name || job.track_id }}
                  <span v-if="job.album_name" class="proxyAlbum"
                    >— {{ job.album_name }}</span
                  >
                </span>
              </div>
              <span class="statusBadge proxyPhase">{{
                formatProxyPhase(job.phase)
              }}</span>
            </div>
            <div v-if="job.total_bytes" class="progressSection">
              <div class="progressBar">
                <div
                  class="progressFill"
                  :style="{ width: proxyProgress(job) + '%' }"
                ></div>
              </div>
              <span class="progressText">
                Downloaded {{ formatBytes(job.bytes_downloaded) }} /
                {{ formatBytes(job.total_bytes) }} · {{ formatProxyRate(job) }}
              </span>
            </div>
            <div
              v-if="job.total_bytes"
              class="progressSection retentionProgress"
            >
              <div class="progressBar">
                <div
                  class="progressFill"
                  :style="{ width: proxyStreamProgress(job) + '%' }"
                ></div>
              </div>
              <span class="progressText">
                Streamed {{ formatBytes(job.bytes_streamed) }} ({{
                  proxyStreamProgress(job)
                }}%)
              </span>
            </div>
            <div class="queueItemDetails">
              <span class="detailItem"
                >Running for {{ formatProxyDuration(job) }}</span
              >
              <span class="detailItem"
                >{{ job.active_streams }} active stream{{
                  job.active_streams === 1 ? "" : "s"
                }}</span
              >
              <span class="detailItem mono">{{ job.track_id }}</span>
            </div>
          </div>
        </div>

        <h3 class="proxySectionTitle">Recent</h3>
        <div v-if="proxyStatus.recent.length === 0" class="emptyState compact">
          No recent proxy downloads.
        </div>
        <div v-else class="queueList">
          <div
            v-for="job in proxyStatus.recent"
            :key="`${job.track_id}-${job.started_at_ms}`"
            class="queueItem proxyJob"
            :class="{ 'status-failed': job.phase === 'failed' }"
          >
            <div class="queueItemHeader">
              <div class="queueItemMain">
                <span class="queueItemType">{{ job.priority }}</span>
                <span
                  class="queueItemName clickable"
                  role="link"
                  tabindex="0"
                  @keydown.enter="goToProxyTrack(job)"
                  @click="goToProxyTrack(job)"
                >
                  {{ job.track_name || job.track_id }}
                  <span v-if="job.album_name" class="proxyAlbum"
                    >— {{ job.album_name }}</span
                  >
                </span>
              </div>
              <span
                class="statusBadge"
                :class="
                  job.phase === 'failed' ? 'status-failed' : 'status-completed'
                "
              >
                {{ formatProxyPhase(job.phase) }}
              </span>
            </div>
            <div class="queueItemDetails">
              <span class="detailItem">{{
                formatBytes(job.bytes_downloaded)
              }}</span>
              <span class="detailItem"
                >{{ proxyStreamProgress(job) }}% streamed</span
              >
              <span class="detailItem">{{ formatProxyDuration(job) }}</span>
              <span class="detailItem">{{
                formatProxyDate(job.finished_at_ms)
              }}</span>
            </div>
            <div v-if="job.error" class="queueItemError">{{ job.error }}</div>
          </div>
        </div>
      </template>
    </div>

    <!-- Queue Tab -->
    <div v-if="activeTab === 'queue'" class="tabContent">
      <div v-if="queueItems.length === 0" class="emptyState">
        Queue is empty.
      </div>
      <div v-else class="queueList">
        <div
          v-for="item in queueItems"
          :key="item.id"
          class="queueItem"
          :class="statusClass(item.status)"
        >
          <div class="queueItemHeader">
            <div class="queueItemMain">
              <span class="queueItemType">{{
                formatContentType(item.content_type)
              }}</span>
              <span
                class="queueItemName clickable"
                role="link"
                tabindex="0"
                @keydown.enter="goToContent(item)"
                @click="goToContent(item)"
              >
                {{ formatItemName(item) }}
                <span class="linkIcon">→</span>
              </span>
            </div>
            <div class="queueItemActions">
              <span class="statusBadge" :class="statusClass(item.status)">
                {{ formatStatus(item.status) }}
              </span>
              <button
                v-if="item.status === 'PENDING'"
                class="uploadButton"
                @click="openUploadModal(item)"
                :disabled="uploadingItems[item.id]"
              >
                {{ uploadingItems[item.id] ? "Uploading..." : "Upload Files" }}
              </button>
              <button
                v-if="item.status === 'FAILED'"
                class="retryButton"
                @click="handleRetry(item.id, false)"
                :disabled="retryingItems[item.id]"
              >
                {{ retryingItems[item.id] ? "..." : "Retry" }}
              </button>
              <button
                v-if="
                  item.status === 'IN_PROGRESS' ||
                  item.status === 'RETRY_WAITING'
                "
                class="forceRetryButton"
                @click="handleRetry(item.id, true)"
                :disabled="retryingItems[item.id]"
              >
                {{ retryingItems[item.id] ? "..." : "Force" }}
              </button>
              <button
                class="deleteButton"
                @click="confirmDelete(item)"
                :disabled="deletingItems[item.id]"
              >
                {{ deletingItems[item.id] ? "..." : "Delete" }}
              </button>
            </div>
          </div>
          <!-- Progress bar for requests with children -->
          <div
            v-if="item.progress && item.progress.total_children > 0"
            class="progressSection"
          >
            <div class="progressBar">
              <div
                class="progressFill"
                :style="{ width: getProgressPercent(item.progress) + '%' }"
                :class="{ 'has-failed': item.progress.failed > 0 }"
              ></div>
            </div>
            <span class="progressText">
              {{ item.progress.completed }}/{{
                item.progress.total_children
              }}
              completed
              <span v-if="item.progress.failed > 0" class="progressFailed">
                ({{ item.progress.failed }} failed)
              </span>
              <span v-if="item.progress.in_progress > 0" class="progressActive">
                ({{ item.progress.in_progress }} active)
              </span>
            </span>
          </div>
          <div class="queueItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Priority:</span>
              <span class="detailValue">{{
                formatPriority(item.priority)
              }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(item.created_at) }}</span>
            </span>
            <span v-if="item.last_attempt_at" class="detailItem">
              <span class="detailLabel">Last attempt:</span>
              <span class="detailValue">{{
                formatDate(item.last_attempt_at)
              }}</span>
            </span>
            <span v-if="item.next_retry_at" class="detailItem">
              <span class="detailLabel">Next retry:</span>
              <span class="detailValue">{{
                formatDate(item.next_retry_at)
              }}</span>
            </span>
            <span v-if="item.retry_count > 0" class="detailItem">
              <span class="detailLabel">Retries:</span>
              <span class="detailValue"
                >{{ item.retry_count }} / {{ item.max_retries }}</span
              >
            </span>
          </div>
          <div
            v-if="item.error_type || item.error_message"
            class="queueItemError"
          >
            <span v-if="item.error_type" class="errorType">{{
              item.error_type
            }}</span>
            <span v-if="item.error_message">{{ item.error_message }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Failed Tab -->
    <div v-if="activeTab === 'failed'" class="tabContent">
      <div v-if="failedItems.length === 0" class="emptyState">
        No failed downloads.
      </div>
      <div v-else class="queueList">
        <div
          v-for="item in failedItems"
          :key="item.id"
          class="queueItem status-failed"
        >
          <div class="queueItemHeader">
            <div class="queueItemMain">
              <span class="queueItemType">{{
                formatContentType(item.content_type)
              }}</span>
              <span
                class="queueItemName clickable"
                role="link"
                tabindex="0"
                @keydown.enter="goToContent(item)"
                @click="goToContent(item)"
              >
                {{ formatItemName(item) }}
                <span class="linkIcon">→</span>
              </span>
            </div>
            <div class="queueItemActions">
              <button
                class="retryButton"
                @click="handleRetry(item.id, false)"
                :disabled="retryingItems[item.id]"
              >
                {{ retryingItems[item.id] ? "..." : "Retry" }}
              </button>
              <button
                class="deleteButton"
                @click="confirmDelete(item)"
                :disabled="deletingItems[item.id]"
              >
                {{ deletingItems[item.id] ? "..." : "Delete" }}
              </button>
            </div>
          </div>
          <!-- Progress info for failed requests with children -->
          <div
            v-if="item.progress && item.progress.total_children > 0"
            class="progressSection"
          >
            <span class="progressText">
              {{ item.progress.completed }}/{{
                item.progress.total_children
              }}
              completed, {{ item.progress.failed }} failed
            </span>
          </div>
          <div class="queueItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(item.created_at) }}</span>
            </span>
            <span v-if="item.last_attempt_at" class="detailItem">
              <span class="detailLabel">Last attempt:</span>
              <span class="detailValue">{{
                formatDate(item.last_attempt_at)
              }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Retries:</span>
              <span class="detailValue"
                >{{ item.retry_count }} / {{ item.max_retries }}</span
              >
            </span>
          </div>
          <div
            v-if="item.error_type || item.error_message"
            class="queueItemError"
          >
            <span v-if="item.error_type" class="errorType">{{
              item.error_type
            }}</span>
            <span v-if="item.error_message">{{ item.error_message }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Downloaded Tab -->
    <div v-if="activeTab === 'downloaded'" class="tabContent">
      <div v-if="completedItems.length === 0" class="emptyState">
        No completed downloads yet.
      </div>
      <div v-else class="queueList">
        <div
          v-for="item in completedItems"
          :key="item.id"
          class="queueItem completed"
        >
          <div class="queueItemMain">
            <span class="queueItemType">{{
              formatContentType(item.content_type)
            }}</span>
            <span
              class="queueItemName clickable"
              role="link"
              tabindex="0"
              @keydown.enter="goToContent(item)"
              @click="goToContent(item)"
            >
              {{ formatItemName(item) }}
              <span class="linkIcon">→</span>
            </span>
          </div>
          <div class="queueItemMeta">
            <span class="statusBadge status-completed">completed</span>
            <span class="queueItemTime">{{
              formatDate(item.completed_at || item.updated_at)
            }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Audit Log Tab -->
    <div v-if="activeTab === 'audit'" class="tabContent">
      <div v-if="auditLog.length === 0" class="emptyState">
        No audit log entries.
      </div>
      <div
        v-else
        class="tableWrapper"
        tabindex="0"
        role="region"
        aria-label="Download audit log"
      >
        <table class="auditTable">
          <thead>
            <tr>
              <th class="colTime">Time</th>
              <th class="colEvent">Event</th>
              <th class="colUser">User</th>
              <th class="colDetails">Details</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="entry in auditLog" :key="entry.id" class="auditRow">
              <td class="colTime">{{ formatDate(entry.timestamp) }}</td>
              <td class="colEvent">
                <span class="eventBadge" :class="eventClass(entry.event_type)">
                  {{ formatEventType(entry.event_type) }}
                </span>
              </td>
              <td class="colUser">
                <span v-if="entry.user_id" class="auditUser">{{
                  entry.user_id
                }}</span>
                <span v-else class="textMuted">—</span>
              </td>
              <td class="colDetails">{{ formatAuditDetails(entry) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Statistics Tab -->
    <div v-if="activeTab === 'statistics'" class="tabContent">
      <!-- Period Selector -->
      <div class="periodSelector">
        <button
          v-for="p in periods"
          :key="p.id"
          class="periodButton"
          :class="{ active: selectedPeriod === p.id }"
          :aria-pressed="selectedPeriod === p.id"
          @click="selectPeriod(p.id)"
        >
          {{ p.label }}
        </button>
      </div>

      <!-- Custom Date Range -->
      <div v-if="selectedPeriod === 'custom'" class="customDateRange">
        <div class="dateInputGroup">
          <label for="customDateFrom">From:</label>
          <input
            id="customDateFrom"
            v-model="customDateFrom"
            type="datetime-local"
            class="dateInput"
            @change="loadStatsHistory"
          />
        </div>
        <div class="dateInputGroup">
          <label for="customDateTo">To:</label>
          <input
            id="customDateTo"
            v-model="customDateTo"
            type="datetime-local"
            class="dateInput"
            @change="loadStatsHistory"
          />
        </div>
        <div class="dateInputGroup">
          <label for="customGranularity">Granularity:</label>
          <select
            id="customGranularity"
            v-model="customGranularity"
            class="granularitySelect"
            @change="loadStatsHistory"
          >
            <option value="hourly">Hourly</option>
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
          </select>
        </div>
      </div>

      <!-- Totals Summary Cards -->
      <div v-if="statsHistory" class="statsTotals">
        <div class="totalCard">
          <span class="totalValue">{{ statsHistory.total_albums }}</span>
          <span class="totalLabel">Albums</span>
        </div>
        <div class="totalCard">
          <span class="totalValue">{{ statsHistory.total_tracks }}</span>
          <span class="totalLabel">Tracks</span>
        </div>
        <div class="totalCard">
          <span class="totalValue">{{ statsHistory.total_images }}</span>
          <span class="totalLabel">Images</span>
        </div>
        <div class="totalCard">
          <span class="totalValue">{{
            formatBytes(statsHistory.total_bytes)
          }}</span>
          <span class="totalLabel">Downloaded</span>
        </div>
        <div class="totalCard totalFailures">
          <span class="totalValue">{{ statsHistory.total_failures }}</span>
          <span class="totalLabel">Failures</span>
        </div>
      </div>

      <!-- Downloads Chart -->
      <div v-if="statsHistory" class="chartSection">
        <h4 class="chartTitle">Downloads Over Time</h4>
        <div class="chartContainer">
          <Line
            v-if="downloadsChartData"
            :data="downloadsChartData"
            :options="lineChartOptions"
          />
          <div v-else class="noData">No data available for this period.</div>
        </div>
      </div>

      <!-- Data Table -->
      <div v-if="statsHistory?.entries?.length > 0" class="tableSection">
        <h4 class="chartTitle">Period Breakdown</h4>
        <div class="tableWrapper">
          <table class="dataTable">
            <thead>
              <tr>
                <th>Period</th>
                <th>Albums</th>
                <th>Tracks</th>
                <th>Images</th>
                <th>Size</th>
                <th>Failures</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="entry in statsHistory.entries"
                :key="entry.period_start"
              >
                <td>{{ formatPeriodDate(entry.period_start) }}</td>
                <td>{{ entry.albums }}</td>
                <td>{{ entry.tracks }}</td>
                <td>{{ entry.images }}</td>
                <td>{{ formatBytes(entry.bytes) }}</td>
                <td :class="{ 'text-danger': entry.failures > 0 }">
                  {{ entry.failures }}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div v-if="!statsHistory && !isLoadingStats" class="emptyState">
        No statistics data available.
      </div>
      <div v-if="isLoadingStats" class="emptyState">Loading statistics...</div>
    </div>

    <!-- Download Request Modal -->
    <div
      v-if="showDownloadModal"
      class="detailOverlay"
      @click.self="closeDownloadModal"
    >
      <div
        class="detailPanel downloadModal"
        role="dialog"
        aria-modal="true"
        aria-label="Download album"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">Download Album</h3>
          <button
            class="closeDetailButton"
            aria-label="Close download dialog"
            @click="closeDownloadModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent">
          <div class="formGroup">
            <label class="formLabel" for="download-album-id">Album ID</label>
            <input
              id="download-album-id"
              v-model="downloadForm.id"
              type="text"
              class="formInput"
              placeholder="Album ID (e.g. Spotify album ID)"
            />
          </div>
          <div class="formGroup">
            <label class="formLabel" for="download-album-name"
              >Album Name</label
            >
            <input
              id="download-album-name"
              v-model="downloadForm.albumName"
              type="text"
              class="formInput"
              placeholder="Album name (for display)"
            />
          </div>
          <div class="formGroup">
            <label class="formLabel" for="download-artist-name"
              >Artist Name</label
            >
            <input
              id="download-artist-name"
              v-model="downloadForm.artistName"
              type="text"
              class="formInput"
              placeholder="Artist name (for display)"
            />
          </div>
          <div v-if="downloadError" class="modalError">
            {{ downloadError }}
          </div>
          <div v-if="downloadSuccess" class="modalSuccess">
            {{ downloadSuccess }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeDownloadModal">
              Cancel
            </button>
            <button
              class="confirmButton"
              @click="submitDownloadRequest"
              :disabled="isSubmitting || !isFormValid"
            >
              {{ isSubmitting ? "Submitting..." : "Download" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Delete Confirmation Modal -->
    <div
      v-if="showDeleteModal"
      class="detailOverlay"
      @click.self="closeDeleteModal"
    >
      <div
        class="detailPanel deleteModal"
        role="dialog"
        aria-modal="true"
        aria-label="Delete download request"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">Delete Download Request</h3>
          <button
            class="closeDetailButton"
            aria-label="Close delete dialog"
            @click="closeDeleteModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent">
          <p class="deleteWarning">
            Are you sure you want to delete this download request?
          </p>
          <div class="deleteItemInfo">
            <span class="queueItemType">{{
              formatContentType(itemToDelete?.content_type)
            }}</span>
            <span class="queueItemName">{{
              formatItemName(itemToDelete)
            }}</span>
          </div>
          <div v-if="deleteError" class="modalError">
            {{ deleteError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeDeleteModal">
              Cancel
            </button>
            <button
              class="deleteConfirmButton"
              @click="executeDelete"
              :disabled="isDeleting"
            >
              {{ isDeleting ? "Deleting..." : "Delete" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Upload Modal -->
    <div
      v-if="showUploadModal"
      class="detailOverlay"
      @click.self="closeUploadModal"
    >
      <div
        class="detailPanel uploadModal"
        role="dialog"
        aria-modal="true"
        aria-label="Upload files"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">
            Upload Files for {{ formatItemName(itemToUpload) }}
          </h3>
          <button
            class="closeDetailButton"
            aria-label="Close upload dialog"
            @click="closeUploadModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent">
          <p class="uploadDescription">
            Upload audio files (ZIP archive or individual audio files) to
            fulfill this download request. The files will be analyzed and
            ingested automatically.
          </p>

          <div
            class="uploadDropzone"
            role="button"
            tabindex="0"
            aria-label="Choose audio files"
            @keydown.enter.self="triggerFileInput"
            @keydown.space.self.prevent="triggerFileInput"
            :class="{ dragging: isDragging }"
            @click="triggerFileInput"
            @dragover.prevent="isDragging = true"
            @dragleave="isDragging = false"
            @drop.prevent="onDrop"
          >
            <input
              ref="fileInput"
              type="file"
              accept=".mp3,.flac,.wav,.ogg,.m4a,.aac,.wma,.opus,.zip"
              @change="handleFileSelect"
              style="display: none"
            />
            <input
              ref="folderInput"
              type="file"
              webkitdirectory
              directory
              @change="handleFolderSelect"
              style="display: none"
            />
            <div class="dropzoneContent">
              <span class="dropzoneIcon">+</span>
              <span class="dropzoneText">
                Drag files/folders here or
                <span class="browseLink">browse files</span>
              </span>
              <span class="dropzoneHint"
                >Supports MP3, FLAC, WAV, OGG, M4A, AAC, OPUS, ZIP, or
                folders</span
              >
              <button class="folderButton" @click.stop="triggerFolderInput">
                Select Folder
              </button>
            </div>
          </div>

          <!-- Upload Progress -->
          <div
            v-if="uploadState.uploading || uploadState.zipping"
            class="uploadProgress"
          >
            <div class="progressBar">
              <div
                class="progressFill"
                :style="{ width: uploadState.progress + '%' }"
              ></div>
            </div>
            <span class="progressText">
              {{ uploadState.zipping ? "Zipping" : "Uploading" }}
              {{ uploadState.filename }}...
            </span>
          </div>

          <div v-if="uploadState.error" class="modalError">
            {{ uploadState.error }}
          </div>

          <div v-if="uploadState.success" class="modalSuccess">
            {{ uploadState.success }}
          </div>

          <div class="modalActions">
            <button
              class="cancelButton"
              @click="closeUploadModal"
              :disabled="uploadState.uploading"
            >
              Close
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted, watch } from "vue";
import { useRouter } from "vue-router";
import { Line } from "vue-chartjs";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from "chart.js";
import { useRemoteStore } from "@/store/remote";
import { useIngestionStore } from "@/store/ingestion";
import JSZip from "jszip";

// Supported audio extensions
const AUDIO_EXTENSIONS = [
  "mp3",
  "flac",
  "wav",
  "ogg",
  "m4a",
  "aac",
  "wma",
  "opus",
];

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
);

const remoteStore = useRemoteStore();
const ingestionStore = useIngestionStore();
const router = useRouter();

// Navigate to album/artist page
const goToContent = (item) => {
  if (item.content_type === "ALBUM") {
    router.push(`/album/${item.content_id}`);
  } else if (item.content_type === "ARTIST") {
    router.push(`/artist/${item.content_id}`);
  }
};

// Download modal state
const showDownloadModal = ref(false);
const downloadModalType = ref(null);
const downloadForm = reactive({
  id: "",
  albumName: "",
  artistName: "",
});
const isSubmitting = ref(false);
const downloadError = ref(null);
const downloadSuccess = ref(null);

const isFormValid = computed(() => {
  if (!downloadForm.id || !downloadForm.artistName) return false;
  if (downloadModalType.value === "album" && !downloadForm.albumName)
    return false;
  return true;
});

const openDownloadModal = (type) => {
  downloadModalType.value = type;
  downloadForm.id = "";
  downloadForm.albumName = "";
  downloadForm.artistName = "";
  downloadError.value = null;
  downloadSuccess.value = null;
  showDownloadModal.value = true;
};

const closeDownloadModal = () => {
  showDownloadModal.value = false;
  downloadModalType.value = null;
};

const submitDownloadRequest = async () => {
  isSubmitting.value = true;
  downloadError.value = null;
  downloadSuccess.value = null;

  const result = await remoteStore.requestAlbumDownload(
    downloadForm.id,
    downloadForm.albumName,
    downloadForm.artistName,
  );

  isSubmitting.value = false;

  if (result.error) {
    downloadError.value =
      typeof result.error === "string"
        ? result.error
        : JSON.stringify(result.error);
  } else {
    downloadSuccess.value = "Album queued for download!";
    await loadData();
    setTimeout(() => {
      closeDownloadModal();
    }, 1500);
  }
};

// Tab and data state
const activeTab = ref("queue");
const isLoading = ref(false);
const loadError = ref(null);

const stats = ref(null);
const proxyStatus = ref(null);
const queueItems = ref([]);
const failedItems = ref([]);
const completedItems = ref([]);
const auditLog = ref([]);
const retryingItems = reactive({});
const deletingItems = reactive({});

// Delete modal state
const showDeleteModal = ref(false);
const itemToDelete = ref(null);
const isDeleting = ref(false);
const deleteError = ref(null);

// Upload modal state
const showUploadModal = ref(false);
const itemToUpload = ref(null);
const fileInput = ref(null);
const folderInput = ref(null);
const isDragging = ref(false);
const uploadingItems = reactive({});

const uploadState = reactive({
  uploading: false,
  zipping: false,
  progress: 0,
  filename: "",
  error: null,
  success: null,
});

// Statistics state
const selectedPeriod = ref("7d");
const statsHistory = ref(null);
const isLoadingStats = ref(false);
const customDateFrom = ref("");
const customDateTo = ref("");
const customGranularity = ref("hourly");

const periods = [
  { id: "24h", label: "Last 24h", seconds: 24 * 3600, granularity: "hourly" },
  {
    id: "7d",
    label: "Last 7 days",
    seconds: 7 * 24 * 3600,
    granularity: "hourly",
  },
  {
    id: "30d",
    label: "Last 30 days",
    seconds: 30 * 24 * 3600,
    granularity: "daily",
  },
  { id: "custom", label: "Custom Range" },
];

const loadStatsHistory = async () => {
  isLoadingStats.value = true;

  const now = Math.floor(Date.now() / 1000);
  let period, since, until;

  if (selectedPeriod.value === "custom") {
    // Use custom granularity for aggregation
    period = customGranularity.value;
    // Convert datetime-local values to unix timestamps
    since = customDateFrom.value
      ? Math.floor(new Date(customDateFrom.value).getTime() / 1000)
      : null;
    until = customDateTo.value
      ? Math.floor(new Date(customDateTo.value).getTime() / 1000)
      : null;
  } else {
    // Find the preset configuration
    const preset = periods.find((p) => p.id === selectedPeriod.value);
    if (preset && preset.seconds) {
      period = preset.granularity;
      since = now - preset.seconds;
      until = null;
    } else {
      period = "daily";
      since = null;
      until = null;
    }
  }

  const result = await remoteStore.fetchDownloadStatsHistory(
    period,
    since,
    until,
  );
  statsHistory.value = result;
  isLoadingStats.value = false;
};

const selectPeriod = async (period) => {
  selectedPeriod.value = period;

  // Initialize custom date range with reasonable defaults
  if (period === "custom" && !customDateFrom.value) {
    const now = new Date();
    const weekAgo = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
    customDateFrom.value = weekAgo.toISOString().slice(0, 16);
    customDateTo.value = now.toISOString().slice(0, 16);
  }

  await loadStatsHistory();
};

// Watch for tab change to load statistics when needed
watch(
  () => activeTab.value,
  async (newTab) => {
    if (newTab === "statistics" && !statsHistory.value) {
      await loadStatsHistory();
    }
  },
);

// Chart configuration
const getEffectiveGranularity = () => {
  if (selectedPeriod.value === "custom") {
    return customGranularity.value;
  }
  const preset = periods.find((p) => p.id === selectedPeriod.value);
  return preset?.granularity || "daily";
};

const formatPeriodDate = (timestamp) => {
  const date = new Date(timestamp * 1000);
  const granularity = getEffectiveGranularity();

  if (granularity === "hourly") {
    return date.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } else if (granularity === "weekly") {
    return `Week of ${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
  }
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
};

const downloadsChartData = computed(() => {
  if (!statsHistory.value?.entries?.length) return null;

  const entries = statsHistory.value.entries;
  const labels = entries.map((e) => formatPeriodDate(e.period_start));

  return {
    labels,
    datasets: [
      {
        label: "Albums",
        data: entries.map((e) => e.albums),
        borderColor: "#1db954",
        backgroundColor: "rgba(29, 185, 84, 0.1)",
        fill: true,
        tension: 0.3,
        yAxisID: "y",
      },
      {
        label: "Tracks",
        data: entries.map((e) => e.tracks),
        borderColor: "#3b82f6",
        backgroundColor: "rgba(59, 130, 246, 0.1)",
        fill: true,
        tension: 0.3,
        yAxisID: "y",
      },
      {
        label: "Failures",
        data: entries.map((e) => e.failures),
        borderColor: "#dc2626",
        backgroundColor: "rgba(220, 38, 38, 0.1)",
        fill: true,
        tension: 0.3,
        yAxisID: "y",
      },
      {
        label: "Bytes (MB)",
        data: entries.map((e) => Math.round(e.bytes / (1024 * 1024))),
        borderColor: "#f59e0b",
        backgroundColor: "rgba(245, 158, 11, 0.1)",
        fill: true,
        tension: 0.3,
        yAxisID: "y1",
      },
    ],
  };
});

const lineChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  interaction: {
    mode: "index",
    intersect: false,
  },
  plugins: {
    legend: {
      position: "top",
      labels: {
        color: "#a1a1aa",
        usePointStyle: true,
        padding: 16,
      },
    },
    tooltip: {
      backgroundColor: "#27272a",
      titleColor: "#fafafa",
      bodyColor: "#a1a1aa",
      borderColor: "#3f3f46",
      borderWidth: 1,
      padding: 12,
    },
  },
  scales: {
    x: {
      grid: {
        color: "rgba(63, 63, 70, 0.3)",
      },
      ticks: {
        color: "#a1a1aa",
        maxRotation: 45,
        minRotation: 45,
      },
    },
    y: {
      type: "linear",
      display: true,
      position: "left",
      title: {
        display: true,
        text: "Count",
        color: "#a1a1aa",
      },
      grid: {
        color: "rgba(63, 63, 70, 0.3)",
      },
      ticks: {
        color: "#a1a1aa",
        precision: 0,
      },
      beginAtZero: true,
    },
    y1: {
      type: "linear",
      display: true,
      position: "right",
      title: {
        display: true,
        text: "MB",
        color: "#f59e0b",
      },
      grid: {
        drawOnChartArea: false,
      },
      ticks: {
        color: "#f59e0b",
        precision: 0,
      },
      beginAtZero: true,
    },
  },
};

const tabs = computed(() => [
  { id: "queue", label: "Queue", count: queueItems.value.length },
  {
    id: "proxy",
    label: "Track Proxy",
    count: proxyStatus.value?.active?.length || 0,
  },
  { id: "failed", label: "Failed", count: failedItems.value.length },
  { id: "downloaded", label: "Downloaded", count: completedItems.value.length },
  { id: "audit", label: "Audit Log" },
  { id: "statistics", label: "Statistics" },
]);

const loadData = async () => {
  isLoading.value = true;
  loadError.value = null;

  try {
    const [
      statsResult,
      proxyResult,
      queueResult,
      failedResult,
      completedResult,
      auditResult,
    ] = await Promise.all([
      remoteStore.fetchDownloadStats(),
      remoteStore.fetchProxyDownloadStatus(),
      remoteStore.fetchDownloadQueue(),
      remoteStore.fetchFailedDownloads(100, 0),
      remoteStore.fetchDownloadCompleted(100, 0),
      remoteStore.fetchDownloadAuditLog(100, 0),
    ]);

    stats.value = statsResult;
    proxyStatus.value = proxyResult;
    // API returns arrays directly, not wrapped in { items: [...] }
    queueItems.value = Array.isArray(queueResult)
      ? queueResult
      : queueResult?.items || [];
    failedItems.value = Array.isArray(failedResult)
      ? failedResult
      : failedResult?.items || [];
    completedItems.value = Array.isArray(completedResult)
      ? completedResult
      : completedResult?.items || [];
    auditLog.value = auditResult?.entries || [];

    if (!statsResult) {
      loadError.value = "Download manager may not be enabled on this server.";
    }
  } catch {
    loadError.value = "Failed to load download manager data.";
  }

  isLoading.value = false;
};

const goToProxyTrack = (job) => {
  router.push(`/track/${job.track_id}`);
};

const formatProxyPhase = (phase) =>
  (phase || "unknown")
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");

const proxyProgress = (job) => {
  if (!job.total_bytes) return 0;
  return Math.min(
    100,
    Math.round((job.bytes_downloaded / job.total_bytes) * 100),
  );
};

const proxyStreamProgress = (job) => {
  if (!job.total_bytes) return 0;
  return Math.min(
    100,
    Math.round((job.bytes_streamed / job.total_bytes) * 100),
  );
};

const formatProxyDuration = (job) => {
  const end = job.finished_at_ms || Date.now();
  const seconds = Math.max(0, Math.round((end - job.started_at_ms) / 1000));
  if (seconds < 60) return `${seconds}s`;
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
};

const formatProxyRate = (job) => {
  const seconds = Math.max(
    0.001,
    (job.updated_at_ms - job.started_at_ms) / 1000,
  );
  return `${formatBytes(job.bytes_downloaded / seconds)}/s`;
};

const formatProxyDate = (timestampMs) =>
  timestampMs ? new Date(timestampMs).toLocaleString() : "—";

const loadProxyStatus = async () => {
  const result = await remoteStore.fetchProxyDownloadStatus();
  if (result) proxyStatus.value = result;
};

const handleRetry = async (itemId, force = false) => {
  retryingItems[itemId] = true;

  const result = await remoteStore.retryDownload(itemId, force);

  if (result.error) {
    alert(result.error);
  } else {
    await loadData();
  }

  retryingItems[itemId] = false;
};

const confirmDelete = (item) => {
  itemToDelete.value = item;
  deleteError.value = null;
  showDeleteModal.value = true;
};

const closeDeleteModal = () => {
  showDeleteModal.value = false;
  itemToDelete.value = null;
  deleteError.value = null;
};

const executeDelete = async () => {
  if (!itemToDelete.value) return;

  const itemId = itemToDelete.value.id;
  isDeleting.value = true;
  deletingItems[itemId] = true;
  deleteError.value = null;

  const result = await remoteStore.deleteDownloadRequest(itemId);

  if (result.error) {
    deleteError.value = result.error;
    isDeleting.value = false;
    deletingItems[itemId] = false;
  } else {
    closeDeleteModal();
    await loadData();
    deletingItems[itemId] = false;
    isDeleting.value = false;
  }
};

const openUploadModal = (item) => {
  itemToUpload.value = item;
  uploadState.uploading = false;
  uploadState.progress = 0;
  uploadState.filename = "";
  uploadState.error = null;
  uploadState.success = null;
  showUploadModal.value = true;
};

const closeUploadModal = () => {
  showUploadModal.value = false;
  itemToUpload.value = null;
  uploadState.uploading = false;
  uploadState.progress = 0;
  uploadState.filename = "";
  uploadState.error = null;
  uploadState.success = null;
};

const triggerFileInput = () => {
  fileInput.value?.click();
};

const triggerFolderInput = () => {
  folderInput.value?.click();
};

const handleFileSelect = (event) => {
  const file = event.target.files?.[0];
  if (file) {
    uploadFile(file);
  }
};

const handleFolderSelect = async (event) => {
  const files = event.target.files;
  if (files && files.length > 0) {
    await uploadFolder(files);
  }
};

// Check if a file is a supported audio format
const isAudioFile = (filename) => {
  const ext = filename.split(".").pop()?.toLowerCase();
  return AUDIO_EXTENSIONS.includes(ext);
};

const onDrop = async (e) => {
  isDragging.value = false;

  // Check if it's a folder drop using DataTransferItemList
  const items = e.dataTransfer?.items;
  if (items && items.length > 0) {
    const firstItem = items[0];

    // Try to get as directory entry (for folder drops)
    if (firstItem.webkitGetAsEntry) {
      const entry = firstItem.webkitGetAsEntry();
      if (entry && entry.isDirectory) {
        await uploadDirectoryEntry(entry);
        return;
      }
    }
  }

  // Fall back to regular file upload
  const file = e.dataTransfer?.files[0];
  if (file) {
    uploadFile(file);
  }
};

// Upload a folder by zipping it first
const uploadFolder = async (files) => {
  if (!itemToUpload.value) return;

  // Filter to only audio files
  const audioFiles = Array.from(files).filter((f) => isAudioFile(f.name));

  if (audioFiles.length === 0) {
    uploadState.error = "No audio files found in folder";
    return;
  }

  // Get folder name from webkitRelativePath
  const folderName =
    audioFiles[0].webkitRelativePath?.split("/")[0] || "folder";

  uploadState.zipping = true;
  uploadState.progress = 0;
  uploadState.filename = folderName;
  uploadState.error = null;
  uploadState.success = null;
  uploadingItems[itemToUpload.value.id] = true;

  try {
    // Create zip
    const zip = new JSZip();

    for (let i = 0; i < audioFiles.length; i++) {
      const file = audioFiles[i];
      const relativePath = file.webkitRelativePath || file.name;
      zip.file(relativePath, file);
      uploadState.progress = Math.round((i / audioFiles.length) * 50);
    }

    // Generate zip blob
    const zipBlob = await zip.generateAsync({ type: "blob" }, (metadata) => {
      uploadState.progress = 50 + Math.round(metadata.percent / 2);
    });

    uploadState.zipping = false;
    uploadState.uploading = true;
    uploadState.progress = 0;

    // Upload the zip
    const zipFile = new File([zipBlob], `${folderName}.zip`, {
      type: "application/zip",
    });

    const result = await remoteStore.uploadIngestionFile(
      zipFile,
      "download_request",
      itemToUpload.value.id,
      (progress) => {
        uploadState.progress = progress;
      },
    );

    if (result.error) {
      uploadState.error = result.error;
    } else {
      // Add session to ingestion store and open monitor modal
      const jobIds = result.job_ids || (result.job_id ? [result.job_id] : []);
      for (const jobId of jobIds) {
        ingestionStore.addSession({
          id: jobId,
          status: "PENDING",
          original_filename: folderName,
        });
        ingestionStore.fetchJobDetails(jobId);
      }
      if (jobIds.length > 0) {
        ingestionStore.openModal(jobIds[0]);
      }
      // Close upload modal and refresh data
      closeUploadModal();
      loadData();
    }
  } catch (error) {
    console.error("[Download Manager] Folder upload error:", error);
    uploadState.error = error.message || "Folder upload failed";
  } finally {
    uploadState.uploading = false;
    uploadState.zipping = false;
    uploadingItems[itemToUpload.value.id] = false;
    if (folderInput.value) {
      folderInput.value.value = "";
    }
  }
};

// Upload a directory from drag & drop using webkitGetAsEntry
const uploadDirectoryEntry = async (dirEntry) => {
  if (!itemToUpload.value) return;

  const folderName = dirEntry.name;

  uploadState.zipping = true;
  uploadState.progress = 0;
  uploadState.filename = folderName;
  uploadState.error = null;
  uploadState.success = null;
  uploadingItems[itemToUpload.value.id] = true;

  try {
    // Recursively read all files from the directory
    const files = await readDirectoryRecursive(dirEntry);
    const audioFiles = files.filter((f) => isAudioFile(f.path));

    if (audioFiles.length === 0) {
      uploadState.error = "No audio files found in folder";
      uploadState.zipping = false;
      uploadingItems[itemToUpload.value.id] = false;
      return;
    }

    // Create zip
    const zip = new JSZip();

    for (let i = 0; i < audioFiles.length; i++) {
      const { path, file } = audioFiles[i];
      zip.file(path, file);
      uploadState.progress = Math.round((i / audioFiles.length) * 50);
    }

    // Generate zip blob
    const zipBlob = await zip.generateAsync({ type: "blob" }, (metadata) => {
      uploadState.progress = 50 + Math.round(metadata.percent / 2);
    });

    uploadState.zipping = false;
    uploadState.uploading = true;
    uploadState.progress = 0;

    // Upload the zip
    const zipFile = new File([zipBlob], `${folderName}.zip`, {
      type: "application/zip",
    });

    const result = await remoteStore.uploadIngestionFile(
      zipFile,
      "download_request",
      itemToUpload.value.id,
      (progress) => {
        uploadState.progress = progress;
      },
    );

    if (result.error) {
      uploadState.error = result.error;
    } else {
      // Add session to ingestion store and open monitor modal
      const jobIds = result.job_ids || (result.job_id ? [result.job_id] : []);
      for (const jobId of jobIds) {
        ingestionStore.addSession({
          id: jobId,
          status: "PENDING",
          original_filename: folderName,
        });
        ingestionStore.fetchJobDetails(jobId);
      }
      if (jobIds.length > 0) {
        ingestionStore.openModal(jobIds[0]);
      }
      // Close upload modal and refresh data
      closeUploadModal();
      loadData();
    }
  } catch (error) {
    console.error("[Download Manager] Directory upload error:", error);
    uploadState.error = error.message || "Directory upload failed";
  } finally {
    uploadState.uploading = false;
    uploadState.zipping = false;
    uploadingItems[itemToUpload.value.id] = false;
  }
};

// Recursively read all files from a directory entry
const readDirectoryRecursive = async (dirEntry, basePath = "") => {
  const files = [];
  const entries = await readDirectoryEntries(dirEntry);

  for (const entry of entries) {
    const path = basePath ? `${basePath}/${entry.name}` : entry.name;

    if (entry.isFile) {
      const file = await getFileFromEntry(entry);
      files.push({ path, file });
    } else if (entry.isDirectory) {
      const subFiles = await readDirectoryRecursive(entry, path);
      files.push(...subFiles);
    }
  }

  return files;
};

// Read all entries from a directory
const readDirectoryEntries = (dirEntry) => {
  return new Promise((resolve, reject) => {
    const reader = dirEntry.createReader();
    const entries = [];

    const readBatch = () => {
      reader.readEntries((batch) => {
        if (batch.length === 0) {
          resolve(entries);
        } else {
          entries.push(...batch);
          readBatch();
        }
      }, reject);
    };

    readBatch();
  });
};

// Get File object from FileEntry
const getFileFromEntry = (fileEntry) => {
  return new Promise((resolve, reject) => {
    fileEntry.file(resolve, reject);
  });
};

const uploadFile = async (file) => {
  if (!itemToUpload.value) return;

  uploadState.uploading = true;
  uploadState.progress = 0;
  uploadState.filename = file.name;
  uploadState.error = null;
  uploadState.success = null;
  uploadingItems[itemToUpload.value.id] = true;

  try {
    // Upload with context_type="download_request" and context_id set to the download request ID
    // Pass progress callback for real-time upload progress tracking
    const result = await remoteStore.uploadIngestionFile(
      file,
      "download_request",
      itemToUpload.value.id,
      (progress) => {
        uploadState.progress = progress;
      },
    );

    if (result.error) {
      uploadState.error = result.error;
    } else {
      // Add session to ingestion store and open monitor modal
      const jobIds = result.job_ids || (result.job_id ? [result.job_id] : []);
      for (const jobId of jobIds) {
        ingestionStore.addSession({
          id: jobId,
          status: "PENDING",
          original_filename: file.name,
        });
        ingestionStore.fetchJobDetails(jobId);
      }
      if (jobIds.length > 0) {
        ingestionStore.openModal(jobIds[0]);
      }
      // Close upload modal and refresh data
      closeUploadModal();
      loadData();
    }
  } catch (error) {
    console.error("[Download Manager] Upload error:", error);
    uploadState.error = error.message || "Upload failed";
  } finally {
    uploadState.uploading = false;
    uploadingItems[itemToUpload.value.id] = false;
    if (fileInput.value) {
      fileInput.value.value = "";
    }
  }
};

const formatItemName = (item) => {
  const name = item.content_name || item.content_id;
  if (item.artist_name) {
    return `${name} - ${item.artist_name}`;
  }
  return name;
};

const formatPriority = (priority) => {
  if (!priority) return "normal";
  const p = priority.toLowerCase();
  if (p === "watchdog") return "high";
  if (p === "user") return "normal";
  if (p === "expansion") return "low";
  return "normal";
};

const formatDate = (timestamp) => {
  if (!timestamp) return "—";
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
};

const formatContentType = (type) => {
  if (!type) return "";
  return type.replace("_", " ").toLowerCase();
};

const formatStatus = (status) => {
  if (!status) return "";
  return status.toLowerCase().replace("_", " ");
};

const statusClass = (status) => {
  switch (status?.toUpperCase()) {
    case "COMPLETED":
      return "status-completed";
    case "IN_PROGRESS":
      return "status-progress";
    case "PENDING":
      return "status-pending";
    case "FAILED":
      return "status-failed";
    case "RETRY_WAITING":
      return "status-retry";
    default:
      return "";
  }
};

const eventClass = (eventType) => {
  if (eventType?.includes("completed") || eventType?.includes("success")) {
    return "event-success";
  }
  if (eventType?.includes("failed") || eventType?.includes("error")) {
    return "event-error";
  }
  if (eventType?.includes("retry")) {
    return "event-retry";
  }
  return "event-info";
};

const formatEventType = (eventType) => {
  if (!eventType) return "—";
  return eventType
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
};

const getProgressPercent = (progress) => {
  if (!progress || progress.total_children === 0) return 0;
  const terminal = progress.completed + progress.failed;
  return Math.round((terminal / progress.total_children) * 100);
};

const formatBytes = (bytes) => {
  if (bytes == null || bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(unitIndex > 0 ? 1 : 0)} ${units[unitIndex]}`;
};

const formatDuration = (ms) => {
  if (ms == null) return "—";
  if (ms < 1000) return `${ms}ms`;
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(1)}s`;
  const mins = Math.floor(secs / 60);
  const remainingSecs = Math.floor(secs % 60);
  return `${mins}m ${remainingSecs}s`;
};

const formatAuditDetails = (entry) => {
  const details = entry.details;
  const contentName = entry.content_id || "";
  const eventType = entry.event_type;

  // Build context prefix from content info
  let prefix = "";
  if (entry.content_type && contentName) {
    const type = entry.content_type.replace("_", " ").toLowerCase();
    prefix = `${type} "${contentName}" — `;
  }

  if (!details) {
    return prefix || "—";
  }

  // Parse details based on event type
  switch (eventType) {
    case "REQUEST_CREATED": {
      const name = details.content_name || contentName;
      const artist = details.artist_name ? ` by ${details.artist_name}` : "";
      const pos =
        details.queue_position != null
          ? `, queue #${details.queue_position}`
          : "";
      return `${name}${artist}${pos}`;
    }

    case "CHILDREN_CREATED": {
      const tracks = details.track_count || 0;
      const images = details.image_count || 0;
      const parts = [];
      if (tracks > 0) parts.push(`${tracks} track${tracks !== 1 ? "s" : ""}`);
      if (images > 0) parts.push(`${images} image${images !== 1 ? "s" : ""}`);
      return `${prefix}spawned ${parts.join(", ") || "no children"}`;
    }

    case "DOWNLOAD_COMPLETED": {
      const size = formatBytes(details.bytes_downloaded);
      const duration = formatDuration(details.duration_ms);
      const tracks = details.tracks_downloaded;
      let result = `${prefix}${size} in ${duration}`;
      if (tracks != null) {
        result += ` (${tracks} track${tracks !== 1 ? "s" : ""})`;
      }
      return result;
    }

    case "DOWNLOAD_FAILED": {
      const errType = details.error_type || "unknown";
      const errMsg = details.error_message || "";
      const retries = details.retry_count || 0;
      return `${prefix}${errType}: ${errMsg} (after ${retries} retries)`;
    }

    case "RETRY_SCHEDULED": {
      const errType = details.error_type || "error";
      const backoff = details.backoff_secs ? `${details.backoff_secs}s` : "?";
      const attempt = details.retry_count || 0;
      return `${prefix}${errType}, retry #${attempt + 1} in ${backoff}`;
    }

    case "ADMIN_RETRY": {
      const prevErr = details.previous_error_type || "unknown";
      return `${prefix}reset from ${prevErr} error`;
    }

    case "WATCHDOG_QUEUED": {
      const reason = details.reason || "missing content";
      return `${prefix}${reason}`;
    }

    case "WATCHDOG_SCAN_STARTED":
      return "Integrity scan started";

    case "WATCHDOG_SCAN_COMPLETED": {
      const total = details.total_missing || 0;
      const queued = details.items_queued || 0;
      const skipped = details.items_skipped || 0;
      const duration = formatDuration(details.scan_duration_ms);
      if (total === 0) {
        return `No issues found (${duration})`;
      }
      return `Found ${total} missing, queued ${queued}, skipped ${skipped} (${duration})`;
    }

    case "DOWNLOAD_STARTED":
      return prefix || "Processing started";

    default:
      // Fallback: show raw details as key-value pairs
      return (
        prefix +
        Object.entries(details)
          .map(([k, v]) => `${k}: ${v}`)
          .join(", ")
      );
  }
};

// Auto-refresh every 10 seconds
const REFRESH_INTERVAL = 10000;
const PROXY_REFRESH_INTERVAL = 2000;
let refreshInterval = null;
let proxyRefreshInterval = null;

onMounted(() => {
  loadData();
  refreshInterval = setInterval(loadData, REFRESH_INTERVAL);
  proxyRefreshInterval = setInterval(loadProxyStatus, PROXY_REFRESH_INTERVAL);
});

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval);
    refreshInterval = null;
  }
  if (proxyRefreshInterval) {
    clearInterval(proxyRefreshInterval);
    proxyRefreshInterval = null;
  }
});
</script>

<style scoped>
.downloadManager {
  width: 100%;
  min-width: 0;
  color: var(--text-base);
}
.pageHeader {
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0 0 10px;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
}
.pageHeader p {
  margin: 0;
  font-size: 14px;
  line-height: 1.5;
  color: var(--text-subdued);
}
button {
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
button:focus-visible,
[tabindex]:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 2px;
}
.actionButtons {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin-bottom: 24px;
}
.actionButton,
.confirmButton {
  min-height: 44px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  background: var(--spotify-green);
  color: #000;
  font-weight: 700;
}
.actionButton:hover:not(:disabled),
.confirmButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}
.refreshButton,
.retryButton,
.forceRetryButton,
.uploadButton,
.deleteButton,
.folderButton {
  min-height: 36px;
  padding: 6px 16px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: var(--text-base);
  font-weight: 600;
  white-space: nowrap;
}
.refreshButton {
  margin-left: auto;
  min-height: 40px;
}
.refreshButton:hover:not(:disabled),
.retryButton:hover:not(:disabled),
.forceRetryButton:hover:not(:disabled),
.uploadButton:hover:not(:disabled),
.deleteButton:hover:not(:disabled),
.folderButton:hover:not(:disabled) {
  border-color: #fff;
  background: #ffffff0c;
}
.deleteButton {
  color: #f3727f;
}
.forceRetryButton {
  color: #f0bc65;
}
.statsSummary,
.proxySummary {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 28px;
  margin-bottom: 24px;
  font-size: 14px;
  color: var(--text-subdued);
}
.statsSummary {
  padding-bottom: 24px;
  border-bottom: 1px solid var(--surface-border);
}
.statItem strong {
  color: var(--text-base);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.statItem.danger strong {
  color: #f3727f;
}
.tabNav,
.periodSelector {
  display: flex;
  gap: 8px;
  max-width: 100%;
  overflow-x: auto;
  padding: 4px 2px;
  margin: -4px -2px 24px;
  scrollbar-width: none;
}
.tabNav::-webkit-scrollbar,
.periodSelector::-webkit-scrollbar {
  display: none;
}
.tabButton,
.periodButton {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
  padding: 8px 16px;
  min-height: 36px;
  border: 0;
  border-radius: 999px;
  background: #242424;
  color: var(--text-base);
}
.tabButton:hover,
.periodButton:hover {
  background: #333;
}
.tabButton.active,
.periodButton.active {
  color: #000;
  background: #fff;
}
.tabCount {
  font-size: 12px;
  opacity: 0.7;
  font-variant-numeric: tabular-nums;
}
.tabContent {
  min-height: 200px;
}
.emptyState {
  display: grid;
  place-items: center;
  min-height: 160px;
  padding: 32px;
  border-radius: 8px;
  background: #181818;
  color: var(--text-subdued);
  font-size: 14px;
  text-align: center;
}
.emptyState.compact {
  min-height: 100px;
}
.queueList {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.queueItem {
  background: #181818;
  border-radius: 8px;
  padding: 20px;
  min-width: 0;
}
.queueItemHeader {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px 24px;
}
.queueItemMain {
  display: flex;
  align-items: baseline;
  gap: 12px;
  min-width: 0;
  flex: 1 1 240px;
}
.queueItemType {
  font-size: 12px;
  color: var(--text-subdued);
  flex-shrink: 0;
  text-transform: capitalize;
}
.queueItemName {
  color: var(--text-base);
  font-size: 16px;
  font-weight: 500;
  overflow-wrap: anywhere;
}
.queueItemName.clickable {
  cursor: pointer;
  border-radius: 2px;
}
.queueItemName.clickable:hover {
  text-decoration: underline;
}
.linkIcon {
  color: var(--text-subdued);
  font-size: 13px;
  margin-left: 4px;
}
.queueItemActions,
.queueItemMeta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}
.queueItemMeta {
  margin-top: 10px;
}
.queueItemDetails {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
  margin-top: 14px;
  font-size: 12px;
}
.detailItem {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 0;
  overflow-wrap: anywhere;
}
.detailLabel,
.detailItem,
.proxyAlbum,
.queueItemArtist,
.queueItemTime,
.textMuted {
  color: var(--text-subdued);
}
.detailValue {
  color: var(--text-base);
}
.queueItemTime {
  font-size: 12px;
}
.queueItemError {
  margin-top: 12px;
  color: #f3727f;
  font-size: 13px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.errorType {
  margin-right: 8px;
  font-weight: 600;
}
.statusBadge,
.eventBadge {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  white-space: nowrap;
  color: var(--text-subdued);
}
.statusBadge::before,
.eventBadge::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  flex-shrink: 0;
}
.statusBadge.status-progress,
.proxyPhase,
.progressActive {
  color: var(--spotify-green);
}
.statusBadge.status-failed,
.event-error,
.progressFailed,
.text-danger {
  color: #f3727f;
}
.statusBadge.status-retry,
.event-retry {
  color: #f0bc65;
}
.progressSection {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 10px 16px;
  margin: 14px 0;
}
.progressBar {
  flex: 1 1 160px;
  max-width: 320px;
  height: 4px;
  background: #535353;
  border-radius: 999px;
  overflow: hidden;
}
.progressFill {
  height: 100%;
  background: var(--spotify-green);
  border-radius: inherit;
}
.retentionProgress .progressFill {
  background: #b3b3b3;
}
.progressFill.has-failed {
  background: #f0bc65;
}
.progressText {
  color: var(--text-subdued);
  font-size: 12px;
  line-height: 1.5;
  font-variant-numeric: tabular-nums;
}
.proxySectionTitle,
.chartTitle {
  margin: 24px 0 16px;
  color: var(--text-base);
  font-size: 18px;
  font-weight: 700;
}
.mono {
  font-family: monospace;
  overflow-wrap: anywhere;
}
.tableWrapper {
  width: 100%;
  overflow-x: auto;
  overscroll-behavior-x: contain;
  border-radius: 4px;
}
.auditTable,
.dataTable {
  width: 100%;
  min-width: 640px;
  border-collapse: collapse;
  font-size: 13px;
}
.auditTable th,
.dataTable th {
  text-align: left;
  padding: 12px 16px;
  color: var(--text-subdued);
  font-weight: 500;
  background: #181818;
  border-bottom: 1px solid var(--surface-border);
}
.auditTable td,
.dataTable td {
  padding: 14px 16px;
  border-bottom: 1px solid var(--surface-border);
  vertical-align: top;
}
.auditRow:hover,
.dataTable tbody tr:hover {
  background: #ffffff08;
}
.colTime {
  width: 160px;
  white-space: nowrap;
  font-size: 12px;
  color: var(--text-subdued);
}
.colEvent {
  width: 160px;
}
.colUser {
  width: 100px;
}
.colDetails {
  overflow-wrap: anywhere;
}
.auditUser {
  color: var(--text-base);
}
.customDateRange {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin-bottom: 24px;
  padding: 20px;
  border-radius: 8px;
  background: #181818;
}
.dateInputGroup {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}
.dateInputGroup label,
.formLabel {
  font-size: 14px;
  color: var(--text-subdued);
}
input,
select {
  min-height: 44px;
  min-width: 0;
  max-width: 100%;
  padding: 10px 12px;
  background: #333;
  border: 1px solid #727272;
  border-radius: 4px;
  color: var(--text-base);
  color-scheme: dark;
  font: inherit;
  font-size: 14px;
}
input:focus-visible,
select:focus-visible {
  outline: 2px solid #fff;
  outline-offset: -2px;
}
.statsTotals {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: 12px;
  margin-bottom: 28px;
}
.totalCard {
  padding: 20px;
  background: #181818;
  border-radius: 8px;
}
.totalValue {
  display: block;
  margin-bottom: 8px;
  font-size: 28px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.totalLabel {
  color: var(--text-subdued);
  font-size: 13px;
}
.totalFailures .totalValue {
  color: #f3727f;
}
.chartSection,
.tableSection {
  margin-bottom: 28px;
  min-width: 0;
}
.chartContainer {
  height: 300px;
  padding: 16px;
  background: #181818;
  border-radius: 8px;
}
.noData {
  display: grid;
  place-items: center;
  height: 100%;
  color: var(--text-subdued);
}
.detailOverlay {
  position: fixed;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: #000b;
  z-index: 1000;
}
.detailPanel {
  display: flex;
  flex-direction: column;
  width: 480px;
  max-width: 100%;
  max-height: calc(100dvh - 32px);
  background: #282828;
  border-radius: 8px;
  box-shadow: var(--shadow-menu);
}
.detailHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  gap: 16px;
  padding: 24px 24px 16px;
}
.detailTitle {
  margin: 0;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}
.closeDetailButton {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  font-size: 28px;
  line-height: 1;
}
.closeDetailButton:hover {
  background: #ffffff12;
  color: #fff;
}
.modalContent {
  padding: 8px 24px 24px;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.formGroup {
  margin-bottom: 20px;
}
.formLabel {
  display: block;
  margin-bottom: 8px;
}
.formInput {
  width: 100%;
}
.modalActions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 24px;
}
.cancelButton,
.deleteConfirmButton {
  min-height: 44px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  font-weight: 700;
}
.cancelButton {
  color: var(--text-subdued);
  background: transparent;
}
.cancelButton:hover {
  color: #fff;
}
.deleteConfirmButton {
  background: #f3727f;
  color: #000;
}
.deleteConfirmButton:hover:not(:disabled) {
  background: #ff8e99;
}
.deleteWarning,
.uploadDescription {
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.6;
  margin: 0 0 20px;
}
.deleteItemInfo {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  padding: 16px;
  background: #181818;
  border-radius: 4px;
}
.errorMessage,
.modalError,
.modalSuccess {
  padding: 12px 16px;
  margin-bottom: 20px;
  border-radius: 4px;
  font-size: 14px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.errorMessage,
.modalError {
  color: #f3727f;
  background: #f3727f12;
}
.modalSuccess {
  color: var(--spotify-green);
  background: #1ed76012;
}
.uploadDropzone {
  padding: 24px 16px;
  border: 1px dashed #727272;
  border-radius: 8px;
  cursor: pointer;
  margin-bottom: 20px;
}
.uploadDropzone:hover,
.uploadDropzone.dragging {
  border-color: #fff;
  background: #ffffff08;
}
.dropzoneContent {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  text-align: center;
}
.dropzoneIcon {
  font-size: 32px;
  color: var(--text-subdued);
}
.dropzoneText {
  font-size: 14px;
}
.browseLink {
  text-decoration: underline;
}
.dropzoneHint {
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-subdued);
}
.uploadProgress {
  margin-bottom: 20px;
}
.uploadProgress .progressBar {
  width: 100%;
  max-width: none;
  margin-bottom: 8px;
}
@media (max-width: 600px) {
  .queueItem {
    padding: 16px;
  }
  .queueItemActions {
    width: 100%;
  }
  .progressBar {
    flex-basis: 100%;
    max-width: none;
  }
  .dateInputGroup {
    width: 100%;
  }
  .detailHeader {
    padding: 20px 16px 12px;
  }
  .modalContent {
    padding: 8px 16px 20px;
  }
  .detailTitle {
    font-size: 22px;
  }
}
</style>
