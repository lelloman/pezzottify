/**
 * Unified Playback Store
 *
 * Single source of truth for all playback state.
 * Delegates to outlets (local/remote) for actual playback.
 *
 * Key benefits:
 * - UI components only interact with this store
 * - No conditional isLocalOutput checks needed in components
 * - Normalized track format regardless of source
 */

import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { useStaticsStore } from "./statics";
import { useDevicesStore } from "./devices";
import { OutletManager } from "./playbackOutlets/OutletManager";

export const usePlaybackStore = defineStore("playback", () => {
  const staticsStore = useStaticsStore();
  const devicesStore = useDevicesStore();

  // ============================================
  // Constants
  // ============================================

  const MAX_PLAYLISTS_HISTORY = 20;
  const PLAYBACK_CONTEXTS = {
    album: "ALBUM",
    userPlaylist: "USER_PLAYLIST",
    userMix: "USER_MIX",
  };

  // ============================================
  // Core playback state
  // ============================================

  // Track & position
  const currentTrackId = ref(null);
  const currentTrackIndex = ref(null);
  const isPlaying = ref(false);
  const progressPercent = ref(0.0);
  const progressSec = ref(0);

  // Volume
  const volume = ref(0.5);
  const muted = ref(false);

  // Queue/playlist
  const playlistsHistory = ref(null);
  const currentPlaylistIndex = ref(null);

  // Output mode
  const outlet = ref("local"); // 'local' or 'remote'
  const remoteDeviceId = ref(null);

  // ============================================
  // Outlet Manager
  // ============================================

  const outletManager = new OutletManager({
    // Callbacks from outlets
    getVolume: () => (muted.value ? 0 : volume.value),
    onTrackEnd: () => skipNextTrack(),
    onPlayStateChange: (playing) => {
      isPlaying.value = playing;
      if (playing && !devicesStore.sessionExists && !devicesStore.isAudioDevice) {
        devicesStore.registerAsAudioDevice();
      }
    },
    onProgressUpdate: (sec, percent) => {
      progressSec.value = sec;
      progressPercent.value = percent;
    },
    onTrackLoaded: () => {
      // Duration available now
    },
    onRemoteStateUpdate: (state) => {
      // Update state from remote
      if (state.is_playing !== undefined) {
        isPlaying.value = state.is_playing;
      }
      if (state.volume !== undefined) {
        volume.value = state.volume;
      }
      if (state.muted !== undefined) {
        muted.value = state.muted;
      }
    },
  });

  // Connect outlet manager to devices store
  outletManager.setDevicesStore(devicesStore);

  // ============================================
  // Computed
  // ============================================

  const currentPlaylist = computed(() => {
    if (currentPlaylistIndex.value !== null && playlistsHistory.value) {
      return playlistsHistory.value[currentPlaylistIndex.value];
    }
    return null;
  });

  const canGoToPreviousPlaylist = computed(
    () => currentPlaylistIndex.value > 0
  );

  const canGoToNextPlaylist = computed(
    () =>
      playlistsHistory.value &&
      currentPlaylistIndex.value < playlistsHistory.value.length - 1
  );

  const isLocalOutput = computed(() => outlet.value === "local");

  // Normalized current track data
  const currentTrack = computed(() => {
    if (outlet.value === "remote") {
      // Get from remote state via devices store, normalize to camelCase
      const state = devicesStore.remoteState;
      const rt = state?.current_track;
      if (!rt) return null;
      return {
        id: rt.id,
        title: rt.title,
        artistsIds: rt.artists_ids || [],
        artistName: rt.artist_name || "Unknown Artist",
        albumId: rt.album_id || "",
        albumTitle: rt.album_title || "Unknown Album",
        duration: rt.duration || 0,
        trackNumber: rt.track_number,
        imageId: rt.image_id || null,
      };
    }

    // Local: resolve from statics
    if (!currentTrackId.value) return null;

    const trackRef = staticsStore.getTrack(currentTrackId.value);
    const track = trackRef?.item;
    if (!track) return null;

    const albumRef = track.album_id
      ? staticsStore.getAlbum(track.album_id)
      : null;
    const album = albumRef?.item;

    const artistId = track.artists_ids?.[0] || track.artist_id;
    const artistRef = artistId ? staticsStore.getArtist(artistId) : null;
    const artist = artistRef?.item;

    return {
      id: track.id,
      title: track.name || track.title,
      artistId: artistId || "",
      artistName: artist?.name || "Unknown Artist",
      albumId: track.album_id || "",
      albumTitle: album?.name || "Unknown Album",
      duration: track.duration || 0,
      trackNumber: track.track_number,
      imageId: album?.image_id || album?.covers?.[0]?.id || null,
    };
  });

  // ============================================
  // Persistence
  // ============================================

  // Load saved state
  const loadPersistedState = () => {
    const savedPlaylistsHistory = localStorage.getItem("playlistsHistory");
    if (savedPlaylistsHistory) {
      playlistsHistory.value = JSON.parse(savedPlaylistsHistory);
      if (playlistsHistory.value && playlistsHistory.value.length > 0) {
        const savedCurrentPlaylistIndex =
          localStorage.getItem("currentPlaylistIndex") ||
          playlistsHistory.value.length - 1;
        currentPlaylistIndex.value = Number.parseInt(savedCurrentPlaylistIndex);

        const loadedTrackIndex = localStorage.getItem("currentTrackIndex");
        if (loadedTrackIndex) {
          const indexValue = Number.parseInt(loadedTrackIndex);
          if (
            Number.isInteger(indexValue) &&
            !Number.isNaN(indexValue) &&
            indexValue >= 0 &&
            indexValue < currentPlaylist.value.tracksIds.length
          ) {
            currentTrackIndex.value = indexValue;
            currentTrackId.value =
              currentPlaylist.value.tracksIds[indexValue];
          }
        }

        const savedPercent = Number.parseFloat(
          localStorage.getItem("progressPercent")
        );
        if (
          !Number.isNaN(savedPercent) &&
          savedPercent >= 0.0 &&
          savedPercent <= 1.0
        ) {
          progressPercent.value = savedPercent;
        }

        const savedSec = Number.parseFloat(localStorage.getItem("progressSec"));
        if (!Number.isNaN(savedSec)) {
          progressSec.value = savedSec;
        }
      }
    }

    const savedMuted = localStorage.getItem("muted");
    if (savedMuted === "true") {
      muted.value = true;
    }

    const savedVolume = localStorage.getItem("volume");
    if (savedVolume) {
      const parseVolume = Number.parseFloat(savedVolume);
      if (!Number.isNaN(parseVolume)) {
        volume.value = Math.max(0.0, Math.min(1.0, parseVolume));
      }
    }
  };

  loadPersistedState();

  // Save state watchers
  const savePlaylistHistory = (history) =>
    localStorage.setItem("playlistsHistory", JSON.stringify(history));

  watch(playlistsHistory, (newHistory) => savePlaylistHistory(newHistory));
  watch(muted, (newMuted) => localStorage.setItem("muted", newMuted));
  watch(volume, (newVolume) => localStorage.setItem("volume", newVolume));
  watch(currentTrackIndex, (newIndex) => {
    if (Number.isInteger(newIndex)) {
      localStorage.setItem("currentTrackIndex", newIndex);
    }
  });
  watch(currentPlaylistIndex, (newIndex) => {
    if (Number.isInteger(newIndex)) {
      localStorage.setItem("currentPlaylistIndex", newIndex);
    }
  });

  let lastSecProgressSaved = 0;
  const persistProgressPercent = () => {
    localStorage.setItem("progressPercent", progressPercent.value);
    lastSecProgressSaved = progressSec.value || 0;
    localStorage.setItem("progressSec", progressSec.value);
  };

  watch(progressSec, (newSec) => {
    if (outlet.value === "remote") return;
    const diff = Math.abs(Math.round(newSec) - lastSecProgressSaved);
    if (diff > 4) {
      persistProgressPercent();
    }
  });

  // ============================================
  // Playlist creation helpers
  // ============================================

  const makePlaylistFromAlbumData = (album) => ({
    context: { name: album.name, id: album.id, edited: false },
    tracksIds: album.discs.flatMap((disc) => disc.tracks),
    type: PLAYBACK_CONTEXTS.album,
  });

  const makePlaylistFromUserPlaylist = (playlist) => ({
    context: { ...playlist, edited: false },
    tracksIds: playlist.tracks.map((t) => t),
    type: PLAYBACK_CONTEXTS.userPlaylist,
  });

  const makePlaylistFromTrackId = (trackId) => ({
    context: { edited: false },
    tracksIds: [trackId],
    type: PLAYBACK_CONTEXTS.userMix,
  });

  const makePlaylistFromTrackIds = (trackIds, name = "Remote Transfer") => ({
    context: { name, id: null, edited: false },
    tracksIds: trackIds,
    type: PLAYBACK_CONTEXTS.userMix,
  });

  // ============================================
  // Playlist management
  // ============================================

  const setNewPlayingPlaylist = (newPlaylist) => {
    let newHistory;

    if (
      playlistsHistory.value &&
      currentPlaylistIndex.value !== null &&
      currentPlaylistIndex.value < playlistsHistory.value.length - 1
    ) {
      newHistory = [
        ...playlistsHistory.value.slice(0, currentPlaylistIndex.value + 1),
        newPlaylist,
      ];
    } else {
      newHistory = [...(playlistsHistory.value || []), newPlaylist];
    }

    if (newHistory.length > MAX_PLAYLISTS_HISTORY) {
      newHistory = newHistory.slice(newHistory.length - MAX_PLAYLISTS_HISTORY);
    }

    playlistsHistory.value = newHistory;
    currentPlaylistIndex.value = newHistory.length - 1;
  };

  const findTrackIndex = (album, discIndex, trackIndex) => {
    let previousDiscsTracks = 0;
    if (discIndex > 0) {
      for (let i = 0; i < discIndex; i++) {
        previousDiscsTracks += album.discs[i].tracks.length;
      }
    }
    return trackIndex + previousDiscsTracks;
  };

  // ============================================
  // Track loading
  // ============================================

  let pendingSeekPercent = null;

  const loadTrack = (index, seekPercent = null) => {
    if (!currentPlaylist.value) return;

    currentTrackIndex.value = index;
    const trackId = currentPlaylist.value.tracksIds[index];
    currentTrackId.value = trackId;

    // Load via outlet manager
    outletManager.loadTrack(
      trackId,
      isPlaying.value,
      seekPercent || pendingSeekPercent
    );
    pendingSeekPercent = null;
  };

  // ============================================
  // Playlist starters
  // ============================================

  const setAlbumId = async (albumId, discIndex, trackIndex) => {
    const album = await Promise.resolve(staticsStore.waitAlbumData(albumId));
    if (album) {
      const albumPlaylist = makePlaylistFromAlbumData(album);
      setNewPlayingPlaylist(albumPlaylist);
      if (Number.isInteger(discIndex) && Number.isInteger(trackIndex)) {
        const desiredTrackIndex = findTrackIndex(album, discIndex, trackIndex);
        loadTrack(desiredTrackIndex);
      } else {
        loadTrack(0);
      }
      play();
    } else {
      console.error("Album", albumId, "not found in staticsStore");
    }
  };

  const setTrack = (newTrack) => {
    const trackPlaylist = makePlaylistFromTrackId(newTrack.id);
    setNewPlayingPlaylist(trackPlaylist);
    loadTrack(0);
    play();
  };

  const setUserPlaylist = async (newPlaylist) => {
    if (newPlaylist.tracks.length === 0) return;
    const userPlaylistPlaylist = makePlaylistFromUserPlaylist(newPlaylist);
    setNewPlayingPlaylist(userPlaylistPlaylist);
    loadTrack(0);
    play();
  };

  const setPlaylistFromTrackIds = (
    trackIds,
    startIndex = 0,
    autoPlay = false
  ) => {
    if (!trackIds || trackIds.length === 0) return;
    const playlist = makePlaylistFromTrackIds(trackIds);
    setNewPlayingPlaylist(playlist);
    loadTrack(startIndex);
    if (autoPlay) {
      play();
    }
  };

  const setPendingTransferSeek = (percentage) => {
    pendingSeekPercent = percentage;
  };

  // ============================================
  // Playback controls
  // ============================================

  const play = () => {
    if (outlet.value === "remote") {
      outletManager.play();
    } else {
      if (currentTrackIndex.value !== null && !outletManager.hasLoadedSound()) {
        loadTrack(currentTrackIndex.value, progressPercent.value);
      }
      outletManager.play();
    }
    isPlaying.value = true;
  };

  const pause = () => {
    outletManager.pause();
    isPlaying.value = false;
  };

  const playPause = () => {
    if (isPlaying.value) {
      pause();
    } else {
      play();
    }
  };

  const setIsPlaying = (newIsPlaying) => {
    if (newIsPlaying) {
      play();
    } else {
      pause();
    }
  };

  const skipNextTrack = () => {
    if (outlet.value === "remote") {
      outletManager.skipNext();
      return;
    }

    const nextIndex = currentTrackIndex.value + 1;
    if (nextIndex >= currentPlaylist.value.tracksIds.length) {
      outletManager.stop();
      isPlaying.value = false;
      progressPercent.value = 0.0;
      progressSec.value = 0;
      return;
    }
    loadTrack(nextIndex);
    if (isPlaying.value) {
      play();
    }
  };

  const skipPreviousTrack = () => {
    if (outlet.value === "remote") {
      outletManager.skipPrevious();
      return;
    }

    const previousIndex = currentTrackIndex.value - 1;
    if (previousIndex < 0) {
      outletManager.stop();
      isPlaying.value = false;
      progressPercent.value = 0.0;
      progressSec.value = 0;
      return;
    }
    loadTrack(previousIndex);
    if (isPlaying.value) {
      play();
    }
  };

  const seekToPercentage = (percentage) => {
    outletManager.seekToPercentage(percentage);
    persistProgressPercent();
  };

  const forward10Sec = () => {
    outletManager.forward10Sec();
  };

  const rewind10Sec = () => {
    outletManager.rewind10Sec();
  };

  const setVolume = (newVolume) => {
    volume.value = newVolume;
    outletManager.setVolume(newVolume);
  };

  const setMuted = (newMuted) => {
    muted.value = newMuted;
    outletManager.setMuted(newMuted, volume.value);
  };

  const loadTrackIndex = (index) => {
    pendingSeekPercent = null;
    if (
      currentPlaylist.value?.tracksIds.length &&
      index >= 0 &&
      index < currentPlaylist.value.tracksIds.length
    ) {
      loadTrack(index);
      if (isPlaying.value) {
        play();
      }
    }
  };

  const stop = () => {
    outletManager.stop();
    isPlaying.value = false;
    progressPercent.value = 0.0;
    progressSec.value = 0;
    currentTrackId.value = null;
    pendingSeekPercent = null;
    currentTrackIndex.value = null;
    currentPlaylistIndex.value = null;
    playlistsHistory.value = [];

    if (devicesStore.isAudioDevice) {
      devicesStore.unregisterAsAudioDevice();
    }
  };

  const suspendForRemote = () => {
    outletManager.stop();
    isPlaying.value = false;
  };

  // ============================================
  // Playlist history navigation
  // ============================================

  const goToPreviousPlaylist = () => {
    if (canGoToPreviousPlaylist.value) {
      currentPlaylistIndex.value -= 1;
      loadTrack(0);
      if (isPlaying.value) {
        play();
      }
    }
  };

  const goToNextPlaylist = () => {
    if (canGoToNextPlaylist.value) {
      currentPlaylistIndex.value += 1;
      loadTrack(0);
      if (isPlaying.value) {
        play();
      }
    }
  };

  // ============================================
  // Queue management
  // ============================================

  const moveTrack = (fromIndex, toIndex) => {
    if (fromIndex === toIndex) return;

    const newTracks = [...currentPlaylist.value.tracksIds];
    const [removedTrack] = newTracks.splice(fromIndex, 1);
    newTracks.splice(toIndex, 0, removedTrack);

    let pushNewHistory = false;
    const newPlaylist = {
      ...currentPlaylist.value,
      context: { ...currentPlaylist.value.context },
      tracksIds: newTracks,
    };

    if (currentPlaylist.value.type === PLAYBACK_CONTEXTS.album) {
      newPlaylist.type = PLAYBACK_CONTEXTS.userMix;
      newPlaylist.context = { name: null, id: null, edited: false };
      pushNewHistory = true;
    } else if (currentPlaylist.value.type === PLAYBACK_CONTEXTS.userPlaylist) {
      newPlaylist.context.edited = true;
    }

    if (pushNewHistory) {
      setNewPlayingPlaylist(newPlaylist);
    } else {
      playlistsHistory.value[currentPlaylistIndex.value] = newPlaylist;
      savePlaylistHistory(playlistsHistory.value);
    }

    // Adjust current track index
    if (fromIndex === currentTrackIndex.value) {
      currentTrackIndex.value = toIndex;
    } else if (
      fromIndex < currentTrackIndex.value &&
      toIndex >= currentTrackIndex.value
    ) {
      currentTrackIndex.value -= 1;
    } else if (
      fromIndex > currentTrackIndex.value &&
      toIndex <= currentTrackIndex.value
    ) {
      currentTrackIndex.value += 1;
    }
    savePlaylistHistory(playlistsHistory.value);
  };

  const addTracksToPlaylist = (tracksIds) => {
    if (!currentPlaylist.value) return;

    let pushNewHistory = false;
    const newTracks = [...currentPlaylist.value.tracksIds, ...tracksIds];
    const newPlaylist = {
      ...currentPlaylist.value,
      context: { ...currentPlaylist.value.context },
      tracksIds: newTracks,
    };

    if (currentPlaylist.value.type === PLAYBACK_CONTEXTS.album) {
      newPlaylist.type = PLAYBACK_CONTEXTS.userMix;
      newPlaylist.context = { name: null, id: null, edited: false };
      pushNewHistory = true;
    } else if (currentPlaylist.value.type === PLAYBACK_CONTEXTS.userPlaylist) {
      newPlaylist.context.edited = true;
    }

    if (pushNewHistory) {
      setNewPlayingPlaylist(newPlaylist);
    } else {
      playlistsHistory.value[currentPlaylistIndex.value] = newPlaylist;
      savePlaylistHistory(playlistsHistory.value);
    }
  };

  const removeTrackFromPlaylist = (index) => {
    if (!currentPlaylist.value) return;

    let pushNewHistory = false;
    const newTracks = [...currentPlaylist.value.tracksIds];
    newTracks.splice(index, 1);

    const newPlaylist = {
      ...currentPlaylist.value,
      context: { ...currentPlaylist.value.context },
      tracksIds: newTracks,
    };

    if (currentPlaylist.value.type === PLAYBACK_CONTEXTS.album) {
      newPlaylist.type = PLAYBACK_CONTEXTS.userMix;
      newPlaylist.context = { name: null, id: null, edited: false };
      pushNewHistory = true;
    } else if (currentPlaylist.value.type === PLAYBACK_CONTEXTS.userPlaylist) {
      newPlaylist.context.edited = true;
    }

    if (index === currentTrackIndex.value) {
      skipNextTrack();
    } else if (index < currentTrackIndex.value) {
      currentTrackIndex.value -= 1;
    }

    if (pushNewHistory) {
      setNewPlayingPlaylist(newPlaylist);
    } else {
      playlistsHistory.value[currentPlaylistIndex.value] = newPlaylist;
      savePlaylistHistory(playlistsHistory.value);
    }
  };

  // ============================================
  // Output device management
  // ============================================

  const selectOutputDevice = (targetDeviceId) => {
    if (targetDeviceId === devicesStore.deviceId || targetDeviceId === null) {
      // Select this device as output (local)
      if (!devicesStore.isAudioDevice) {
        devicesStore.requestBecomeAudioDevice();
      }
      switchToLocalOutput();
    } else {
      // Select remote device as output
      switchToRemoteOutput(targetDeviceId);
    }
  };

  const switchToLocalOutput = () => {
    if (outlet.value === "local") return;

    const wasRemote = outlet.value === "remote";
    outlet.value = "local";
    remoteDeviceId.value = null;

    // When returning from remote with no loaded sound, restore persisted
    // progress instead of carrying over the remote position values.
    if (wasRemote && !outletManager.hasLoadedSound()) {
      const savedPercent = Number.parseFloat(
        localStorage.getItem("progressPercent")
      );
      if (!Number.isNaN(savedPercent) && savedPercent >= 0.0 && savedPercent <= 1.0) {
        progressPercent.value = savedPercent;
      }
      const savedSec = Number.parseFloat(localStorage.getItem("progressSec"));
      if (!Number.isNaN(savedSec)) {
        progressSec.value = savedSec;
      }
      isPlaying.value = false;
    }

    outletManager.switchToLocal({
      trackId: currentTrackId.value,
      position: progressSec.value,
      isPlaying: isPlaying.value,
      volume: volume.value,
      muted: muted.value,
    });
  };

  const switchToRemoteOutput = (deviceId) => {
    outlet.value = "remote";
    remoteDeviceId.value = deviceId;
    outletManager.switchToRemote();
  };

  // ============================================
  // Devices store callbacks
  // ============================================

  devicesStore.setPlaybackCallbacks({
    onWelcome: (payload) => {
      if (payload.session.exists && payload.session.state) {
        const currentAudioDevice = devicesStore.devices.find(
          (d) => d.is_audio_device
        );
        if (currentAudioDevice) {
          if (currentAudioDevice.id === payload.device_id) {
            // We are the audio device
            switchToLocalOutput();
            devicesStore.registerAsAudioDevice();
          } else {
            // Another device is the audio device
            switchToRemoteOutput(currentAudioDevice.id);
            // Feed the initial state to the remote outlet
            outletManager.updateRemoteState(payload.session.state);
          }
        }
      }

      // Check if we should reclaim (only if no device is currently playing)
      if (devicesStore.reclaimable && currentTrackId.value && !devicesStore.audioDevice) {
        devicesStore.reclaimAudioDevice(buildPlaybackState());
      }
    },

    onRemoteState: (state) => {
      outletManager.updateRemoteState(state);

      // Update local state from remote
      if (outlet.value === "remote") {
        isPlaying.value = state.is_playing;
        if (state.volume !== undefined) {
          volume.value = state.volume;
        }
        if (state.muted !== undefined) {
          muted.value = state.muted;
        }
      }
    },

    onQueueSync: () => {
      // Could update queue display here if needed
    },

    onSessionEnded: () => {
      switchToLocalOutput();
    },

    onDeviceListChanged: () => {
      const currentAudioDevice = devicesStore.devices.find(
        (d) => d.is_audio_device
      );
      if (currentAudioDevice) {
        if (currentAudioDevice.id === devicesStore.deviceId) {
          // We are now the audio device
          switchToLocalOutput();
        } else if (remoteDeviceId.value !== currentAudioDevice.id) {
          // Another device is now the audio device
          switchToRemoteOutput(currentAudioDevice.id);
        }
      }
    },

    onCommand: (payload) => {
      // Handle commands when we're the audio device
      const { command, payload: cmdPayload } = payload;

      switch (command) {
        case "play":
          play();
          break;
        case "pause":
          pause();
          break;
        case "seek":
          if (cmdPayload?.position !== undefined) {
            const track = currentTrack.value;
            const duration = track?.duration || 0;
            if (duration > 0) {
              seekToPercentage(cmdPayload.position / duration);
            }
          }
          break;
        case "next":
          skipNextTrack();
          break;
        case "prev":
          skipPreviousTrack();
          break;
        case "setVolume":
          if (cmdPayload?.volume !== undefined) {
            setVolume(cmdPayload.volume);
          }
          break;
        case "setMuted":
          if (cmdPayload?.muted !== undefined) {
            setMuted(cmdPayload.muted);
          }
          break;
      }
    },

    onPrepareTransfer: (payload) => {
      const state = buildPlaybackState();
      const queue = buildQueueItems();
      pause();
      devicesStore.sendTransferReady(payload.transfer_id, state, queue);
    },

    onBecomeAudioDevice: async (payload) => {
      // Apply received state
      await applyRemoteStateToLocal(payload.state, payload.queue);
      devicesStore.confirmTransferComplete(payload.transfer_id);
      switchToLocalOutput();

      if (payload.state.is_playing) {
        play();
      }
    },

    onTransferComplete: () => {
      // We were the source, suspend local audio but preserve playlist state
      suspendForRemote();
      const newAudioDevice = devicesStore.devices.find(
        (d) => d.is_audio_device
      );
      if (newAudioDevice) {
        switchToRemoteOutput(newAudioDevice.id);
      }
    },

    onTransferAborted: () => {
      // If we were source, resume
      if (devicesStore.isAudioDevice) {
        play();
      }
    },

    getPlaybackState: () => buildPlaybackState(),
  });

  // ============================================
  // State building helpers
  // ============================================

  const buildPlaybackState = () => {
    let currentTrackData = null;

    if (currentTrackId.value) {
      const trackRef = staticsStore.getTrack(currentTrackId.value);
      const track = trackRef?.item;

      if (track) {
        const albumRef = track.album_id
          ? staticsStore.getAlbum(track.album_id)
          : null;
        const album = albumRef?.item;

        const artistId = track.artists_ids?.[0] || track.artist_id;
        const artistRef = artistId ? staticsStore.getArtist(artistId) : null;
        const artist = artistRef?.item;

        currentTrackData = {
          id: track.id,
          title: track.name || track.title,
          artist_id: artistId || "",
          artist_name: artist?.name || "Unknown Artist",
          artists_ids: track.artists_ids || (artistId ? [artistId] : []),
          album_id: track.album_id || "",
          album_title: album?.name || "Unknown Album",
          duration: track.duration || 0,
          track_number: track.track_number,
          image_id: album?.image_id || album?.covers?.[0]?.id || null,
        };
      }
    }

    return {
      current_track: currentTrackData,
      queue_position: currentTrackIndex.value || 0,
      queue_version: devicesStore.queueVersion,
      position: progressSec.value || 0,
      is_playing: isPlaying.value,
      volume: volume.value,
      muted: muted.value || false,
      shuffle: false,
      repeat: "off",
      timestamp: Date.now(),
    };
  };

  const buildQueueItems = () => {
    if (!currentPlaylist.value?.tracksIds) {
      return [];
    }
    return currentPlaylist.value.tracksIds.map((id) => ({
      id,
      added_at: Date.now(),
    }));
  };

  const applyRemoteStateToLocal = async (state, queue) => {
    if (!queue || queue.length === 0) return;

    const trackIds = queue.map((item) => item.id);

    // Wait for tracks to be loaded
    for (const id of trackIds) {
      await staticsStore.waitTrackData(id);
    }

    const startIndex =
      state.queue_position >= 0 && state.queue_position < trackIds.length
        ? state.queue_position
        : 0;

    // Store pending seek
    if (state.position > 0) {
      const track = await staticsStore.waitTrackData(trackIds[startIndex]);
      if (track?.duration) {
        pendingSeekPercent = state.position / track.duration;
      }
    }

    setPlaylistFromTrackIds(trackIds, startIndex, false);

    if (state.volume !== undefined) {
      setVolume(state.volume);
    }
    if (state.muted !== undefined) {
      setMuted(state.muted);
    }
  };

  // ============================================
  // Watch for state changes to broadcast
  // ============================================

  let lastBroadcastTime = 0;
  const MIN_BROADCAST_INTERVAL = 500;

  const throttledBroadcast = () => {
    if (!devicesStore.isAudioDevice) return;
    const now = Date.now();
    if (now - lastBroadcastTime < MIN_BROADCAST_INTERVAL) return;
    lastBroadcastTime = now;
    devicesStore.broadcastStateNow();
  };

  watch(() => isPlaying.value, (playing) => {
    throttledBroadcast();
    if (devicesStore.isAudioDevice) {
      if (playing) {
        devicesStore.startPositionBroadcast();
      } else {
        devicesStore.stopPositionBroadcast();
      }
    }
  });
  watch(() => currentTrackId.value, throttledBroadcast);
  watch(() => currentTrackIndex.value, throttledBroadcast);

  watch(
    () => currentPlaylist.value?.tracksIds?.length,
    () => {
      if (devicesStore.isAudioDevice) {
        devicesStore.broadcastQueueUpdate(buildQueueItems());
      }
    }
  );

  // ============================================
  // Exports
  // ============================================

  return {
    // Core state
    currentTrackId,
    currentTrackIndex,
    currentPlaylist,
    isPlaying,
    progressPercent,
    progressSec,
    volume,
    muted,
    outlet,
    remoteDeviceId,

    // Computed
    currentTrack,
    canGoToPreviousPlaylist,
    canGoToNextPlaylist,
    isLocalOutput,

    // Constants
    PLAYBACK_CONTEXTS,

    // Playlist starters
    setAlbumId,
    setTrack,
    setUserPlaylist,
    setPlaylistFromTrackIds,
    setPendingTransferSeek,

    // Playback controls
    play,
    pause,
    playPause,
    setIsPlaying,
    skipNextTrack,
    skipPreviousTrack,
    seekToPercentage,
    forward10Sec,
    rewind10Sec,
    setVolume,
    setMuted,
    stop,
    loadTrackIndex,

    // Playlist navigation
    goToPreviousPlaylist,
    goToNextPlaylist,

    // Queue management
    moveTrack,
    addTracksToPlaylist,
    removeTrackFromPlaylist,

    // Output device
    selectOutputDevice,
  };
});
