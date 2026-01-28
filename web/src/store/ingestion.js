/**
 * Ingestion Monitor Store
 *
 * Manages ingestion job sessions, real-time updates, and modal state.
 * Provides unified state for the ingestion monitor modal.
 */

import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { useRemoteStore } from "./remote";
import { registerHandler, unregisterHandler } from "../services/websocket";

export const useIngestionStore = defineStore("ingestion", () => {
  const remoteStore = useRemoteStore();

  // =====================================================
  // State
  // =====================================================

  /**
   * Map of job_id -> session data
   * Session: { job, files, candidates, review, dismissed }
   */
  const sessions = ref(new Map());

  /** Whether the ingestion monitor modal is open */
  const isModalOpen = ref(false);

  /** Currently active tab (job_id) in the modal */
  const activeTabId = ref(null);

  /** Whether the store has been initialized */
  const isInitialized = ref(false);

  /** Whether a review is being resolved */
  const isResolving = ref(false);

  /** Error from last resolve attempt */
  const resolveError = ref(null);

  // =====================================================
  // Computed
  // =====================================================

  /** Visible (non-dismissed) sessions */
  const visibleSessions = computed(() => {
    return [...sessions.value.values()].filter((s) => !s.dismissed);
  });

  /** Count of active (non-terminal) jobs */
  const activeCount = computed(() => {
    const terminalStates = ["COMPLETED", "FAILED"];
    return visibleSessions.value.filter(
      (s) => !terminalStates.includes(s.job?.status),
    ).length;
  });

  /** Count of jobs needing review */
  const reviewCount = computed(() => {
    return visibleSessions.value.filter(
      (s) => s.job?.status === "AWAITING_REVIEW" && s.review,
    ).length;
  });

  /** Count of completed jobs (not yet dismissed) */
  const completedCount = computed(() => {
    return visibleSessions.value.filter(
      (s) => s.job?.status === "COMPLETED",
    ).length;
  });

  /**
   * Badge state for TopBar:
   * - "hidden": no visible sessions
   * - "active": uploads in progress (blue/pulsing)
   * - "review": review pending (yellow)
   * - "complete": all done (green)
   */
  const badgeState = computed(() => {
    if (visibleSessions.value.length === 0) return "hidden";
    if (reviewCount.value > 0) return "review";
    if (activeCount.value > 0) return "active";
    if (completedCount.value > 0) return "complete";
    return "hidden";
  });

  /** Get the currently active session (for modal display) */
  const activeSession = computed(() => {
    if (!activeTabId.value) return null;
    return sessions.value.get(activeTabId.value) || null;
  });

  // =====================================================
  // WebSocket Handlers
  // =====================================================

  /**
   * Handle incoming ingestion WebSocket messages.
   * Message types: ingestion_update, ingestion_progress, ingestion_match_found,
   *                ingestion_review_needed, ingestion_completed, ingestion_failed
   */
  function handleIngestionMessage(type, payload) {
    const jobId = payload.job_id;
    if (!jobId) return;

    const session = sessions.value.get(jobId);
    if (!session) {
      // Create a new session for this job if we don't have one
      addSession({ id: jobId, status: payload.status || "PENDING" });
      // Fetch full details
      fetchJobDetails(jobId);
      return;
    }

    switch (type) {
      case "ingestion_update":
        // General status update
        if (session.job) {
          session.job.status = payload.status;
          if (payload.matched_track_id) {
            session.job.matched_album_id = payload.matched_track_id;
          }
          if (payload.match_confidence !== undefined) {
            session.job.match_confidence = payload.match_confidence;
          }
          if (payload.error_message) {
            session.job.error_message = payload.error_message;
          }
          if (payload.needs_review) {
            // Fetch review details
            fetchJobDetails(jobId);
          }
        }
        break;

      case "ingestion_progress":
        // Progress update during processing
        {
          const previousStatus = session.job?.status;
          if (session.job) {
            session.job.status = payload.status;
          }
          session.phase = payload.phase;
          session.phaseProgress = payload.phase_progress;
          session.filesProcessed = payload.files_processed;
          session.filesTotal = payload.files_total;

          // Re-fetch details when transitioning to CONVERTING or COMPLETED
          // to get updated file data (matched_track_id, converted status, etc.)
          const needsRefresh =
            (previousStatus === "MAPPING_TRACKS" && payload.status === "CONVERTING") ||
            (previousStatus !== "COMPLETED" && payload.status === "COMPLETED");
          if (needsRefresh) {
            fetchJobDetails(jobId);
          }
        }
        break;

      case "ingestion_match_found":
        // Album match found
        if (session.job) {
          session.job.status =
            payload.ticket_type === "SUCCESS" ? "MAPPING_TRACKS" : "AWAITING_REVIEW";
          session.job.matched_album_id = payload.album_id;
          session.job.detected_album = payload.album_name;
          session.job.detected_artist = payload.artist_name;
          session.job.match_confidence = payload.confidence;
        }
        session.candidates = payload.candidates || [];
        session.ticketType = payload.ticket_type;
        // Fetch full details to get candidates with all metadata
        fetchJobDetails(jobId);
        break;

      case "ingestion_review_needed":
        // Review is needed
        if (session.job) {
          session.job.status = "AWAITING_REVIEW";
        }
        session.review = {
          question: payload.question,
          options: payload.options,
        };
        break;

      case "ingestion_completed":
        // Job completed successfully
        if (session.job) {
          session.job.status = "COMPLETED";
          session.job.tracks_converted = payload.tracks_added;
          session.job.detected_album = payload.album_name;
          session.job.detected_artist = payload.artist_name;
        }
        session.phase = "complete";
        session.phaseProgress = 100;
        // Fetch final state with all file updates
        fetchJobDetails(jobId);
        break;

      case "ingestion_failed":
        // Job failed
        if (session.job) {
          session.job.status = "FAILED";
          session.job.error_message = payload.error;
        }
        break;
    }

    // Trigger reactivity
    sessions.value = new Map(sessions.value);
  }

  // =====================================================
  // Actions
  // =====================================================

  /**
   * Initialize the store and register WebSocket handlers.
   */
  function initialize() {
    if (isInitialized.value) return;

    // Register for all ingestion WebSocket messages using "ingestion" prefix
    // Note: The WS router uses type.split(".")[0] as prefix, but our types use underscores
    // So we register each full type name as handlers are keyed by the full prefix
    // (ingestion_update, ingestion_progress, etc. each become their own prefix)
    const ingestionTypes = [
      "ingestion_update",
      "ingestion_progress",
      "ingestion_match_found",
      "ingestion_review_needed",
      "ingestion_completed",
      "ingestion_failed",
    ];

    for (const msgType of ingestionTypes) {
      registerHandler(msgType, (type, payload) =>
        handleIngestionMessage(type, payload),
      );
    }

    isInitialized.value = true;
  }

  /**
   * Cleanup on logout or unmount.
   */
  function cleanup() {
    const ingestionTypes = [
      "ingestion_update",
      "ingestion_progress",
      "ingestion_match_found",
      "ingestion_review_needed",
      "ingestion_completed",
      "ingestion_failed",
    ];

    for (const msgType of ingestionTypes) {
      unregisterHandler(msgType);
    }

    sessions.value.clear();
    isModalOpen.value = false;
    activeTabId.value = null;
    isInitialized.value = false;
  }

  /**
   * Add a new session for a job.
   * @param {Object} job - Minimal job object with at least { id, status }
   */
  function addSession(job) {
    if (!job?.id) return;

    const existingSession = sessions.value.get(job.id);
    if (existingSession) {
      // Update existing session
      existingSession.job = { ...existingSession.job, ...job };
      sessions.value = new Map(sessions.value);
      return;
    }

    // Create new session
    const session = {
      job: job,
      files: [],
      candidates: [],
      review: null,
      dismissed: false,
      phase: null,
      phaseProgress: 0,
      filesProcessed: 0,
      filesTotal: job.file_count || 0,
      ticketType: null,
    };

    sessions.value.set(job.id, session);
    sessions.value = new Map(sessions.value);

    // Set as active tab if modal is open and no active tab
    if (isModalOpen.value && !activeTabId.value) {
      activeTabId.value = job.id;
    }
  }

  /**
   * Fetch detailed job information from the server.
   * @param {string} jobId - Job ID
   */
  async function fetchJobDetails(jobId) {
    const details = await remoteStore.fetchIngestionJobDetails(jobId);
    if (!details) return;

    const session = sessions.value.get(jobId);
    if (session) {
      session.job = details.job;
      session.files = details.files || [];
      session.candidates = details.candidates || [];
      session.review = details.review;
      session.filesTotal = details.job?.file_count || 0;
      sessions.value = new Map(sessions.value);
    } else {
      // Create new session with full details
      addSession(details.job);
      const newSession = sessions.value.get(jobId);
      if (newSession) {
        newSession.files = details.files || [];
        newSession.candidates = details.candidates || [];
        newSession.review = details.review;
        sessions.value = new Map(sessions.value);
      }
    }
  }

  /**
   * Dismiss a session (hide from UI but keep tracking).
   * @param {string} jobId - Job ID
   */
  function dismissSession(jobId) {
    const session = sessions.value.get(jobId);
    if (session) {
      session.dismissed = true;
      sessions.value = new Map(sessions.value);

      // Switch to another tab if this was active
      if (activeTabId.value === jobId) {
        const remaining = visibleSessions.value;
        activeTabId.value = remaining.length > 0 ? remaining[0].job.id : null;
      }
    }
  }

  /**
   * Completely remove a session.
   * @param {string} jobId - Job ID
   */
  function removeSession(jobId) {
    sessions.value.delete(jobId);
    sessions.value = new Map(sessions.value);

    if (activeTabId.value === jobId) {
      const remaining = visibleSessions.value;
      activeTabId.value = remaining.length > 0 ? remaining[0].job.id : null;
    }
  }

  /**
   * Open the ingestion monitor modal.
   * @param {string|null} jobId - Optional job ID to focus
   */
  function openModal(jobId = null) {
    isModalOpen.value = true;

    if (jobId && sessions.value.has(jobId)) {
      activeTabId.value = jobId;
    } else if (!activeTabId.value && visibleSessions.value.length > 0) {
      activeTabId.value = visibleSessions.value[0].job.id;
    }
  }

  /**
   * Close the ingestion monitor modal.
   */
  function closeModal() {
    isModalOpen.value = false;
  }

  /**
   * Switch to a different tab in the modal.
   * @param {string} jobId - Job ID
   */
  function setActiveTab(jobId) {
    if (sessions.value.has(jobId)) {
      activeTabId.value = jobId;
    }
  }

  /**
   * Resolve a review by selecting an option.
   * @param {string} jobId - Job ID
   * @param {string} optionId - Selected option ID
   */
  async function resolveReview(jobId, optionId) {
    isResolving.value = true;
    resolveError.value = null;

    try {
      const result = await remoteStore.resolveIngestionReview(jobId, optionId);
      if (result && !result.error) {
        const session = sessions.value.get(jobId);
        if (session) {
          session.review = null;
          // Update job with new status from response
          if (result.job) {
            session.job = result.job;
          }
          sessions.value = new Map(sessions.value);
        }
        return result;
      } else {
        resolveError.value = result?.error || "Failed to resolve review";
        return result;
      }
    } catch (e) {
      resolveError.value = e.message || "Failed to resolve review";
      return { error: resolveError.value };
    } finally {
      isResolving.value = false;
    }
  }

  // =====================================================
  // Return
  // =====================================================

  return {
    // State
    sessions,
    isModalOpen,
    activeTabId,
    isInitialized,
    isResolving,
    resolveError,

    // Computed
    visibleSessions,
    activeCount,
    reviewCount,
    completedCount,
    badgeState,
    activeSession,

    // Actions
    initialize,
    cleanup,
    addSession,
    fetchJobDetails,
    dismissSession,
    removeSession,
    openModal,
    closeModal,
    setActiveTab,
    resolveReview,
  };
});
