/**
 * Playback Store
 *
 * Single source of truth for all playback state.
 * Uses LocalOutlet (Howler.js) directly for audio playback.
 */

import { defineStore } from "pinia";
import { computed, ref, shallowRef, watch } from "vue";
import { useStaticsStore } from "./statics";
import { LocalOutlet } from "./playbackOutlets/LocalOutlet";

export const usePlaybackStore = defineStore("playback", () => {
  const staticsStore = useStaticsStore();

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

  const isPlaying = ref(false);
  const progressSec = ref(0);
  const progressPercent = ref(0.0);
  const localDuration = ref(0);
  const currentTrack = shallowRef(null);

  // Track & position
  const currentTrackId = ref(null);
  const currentTrackIndex = ref(null);

  // Volume
  const volume = ref(0.5);
  const muted = ref(false);

  // Queue/playlist
  const playlistsHistory = ref(null);
  const currentPlaylistIndex = ref(null);

  // ============================================
  // Local Outlet
  // ============================================

  const localOutlet = new LocalOutlet({
    getVolume: () => (muted.value ? 0 : volume.value),
    onTrackEnd: () => skipNextTrack(),
    onPlayStateChange: (playing) => {
      isPlaying.value = playing;
    },
    onProgressUpdate: (sec, percent) => {
      progressSec.value = sec;
      progressPercent.value = percent;
    },
    onTrackLoaded: (duration) => {
      localDuration.value = duration || 0;
    },
  });

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

  // ============================================
  // Local track resolution
  // ============================================

  const resolveLocalTrack = () => {
    if (!currentTrackId.value) {
      currentTrack.value = null;
      return;
    }

    const trackRef = staticsStore.getTrack(currentTrackId.value);
    const track = trackRef?.item;
    if (!track) {
      currentTrack.value = null;
      return;
    }

    const albumRef = track.album_id
      ? staticsStore.getAlbum(track.album_id)
      : null;
    const album = albumRef?.item;

    const artistId = track.artists_ids?.[0] || track.artist_id;
    const artistRef = artistId ? staticsStore.getArtist(artistId) : null;
    const artist = artistRef?.item;

    currentTrack.value = {
      id: track.id,
      title: track.name || track.title,
      artistId: artistId || "",
      artistName: artist?.name || "Unknown Artist",
      albumId: track.album_id || "",
      albumTitle: album?.name || "Unknown Album",
      duration: track.duration || localDuration.value || 0,
      trackNumber: track.track_number,
      imageId: album?.image_id || album?.covers?.[0]?.id || null,
    };
  };

  watch(() => currentTrackId.value, resolveLocalTrack);

  // Re-resolve when statics data loads (artist/album names become available)
  watch(
    () => {
      if (!currentTrackId.value) return null;
      const t = staticsStore.getTrack(currentTrackId.value);
      const track = t?.item;
      if (!track) return null;
      const albumRef = track.album_id
        ? staticsStore.getAlbum(track.album_id)
        : null;
      const artistId = track.artists_ids?.[0] || track.artist_id;
      const artistRef = artistId ? staticsStore.getArtist(artistId) : null;
      return {
        trackName: track.name || track.title,
        albumName: albumRef?.item?.name,
        artistName: artistRef?.item?.name,
        imageId: albumRef?.item?.image_id || albumRef?.item?.covers?.[0]?.id,
      };
    },
    () => resolveLocalTrack(),
    { deep: true }
  );

  // ============================================
  // Persistence
  // ============================================

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

  const makePlaylistFromTrackIds = (trackIds, name = "Mix") => ({
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

    localOutlet.loadTrack(
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
    if (!album) {
      console.error("Album", albumId, "not found in staticsStore");
      return;
    }

    let startIndex = 0;
    if (Number.isInteger(discIndex) && Number.isInteger(trackIndex)) {
      startIndex = findTrackIndex(album, discIndex, trackIndex);
    }

    const albumPlaylist = makePlaylistFromAlbumData(album);
    setNewPlayingPlaylist(albumPlaylist);
    loadTrack(startIndex);
    play();
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

  // ============================================
  // Playback controls
  // ============================================

  const play = () => {
    if (currentTrackIndex.value !== null && !localOutlet.hasLoadedSound()) {
      loadTrack(currentTrackIndex.value, progressPercent.value);
    }
    localOutlet.play();
    isPlaying.value = true;
  };

  const pause = () => {
    localOutlet.pause();
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
    const nextIndex = currentTrackIndex.value + 1;
    if (nextIndex >= currentPlaylist.value.tracksIds.length) {
      localOutlet.stop();
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
    const previousIndex = currentTrackIndex.value - 1;
    if (previousIndex < 0) {
      localOutlet.stop();
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
    localOutlet.seekToPercentage(percentage);
    persistProgressPercent();
  };

  const forward10Sec = () => {
    const pos = localOutlet.getPosition();
    localOutlet.seekTo(pos + 10);
  };

  const rewind10Sec = () => {
    const pos = localOutlet.getPosition();
    localOutlet.seekTo(Math.max(0, pos - 10));
  };

  const setVolume = (newVolume) => {
    volume.value = newVolume;
    localOutlet.setVolume(newVolume);
  };

  const setMuted = (newMuted) => {
    muted.value = newMuted;
    localOutlet.setMuted(newMuted, volume.value);
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
    localOutlet.stop();
    isPlaying.value = false;
    progressPercent.value = 0.0;
    progressSec.value = 0;
    currentTrackId.value = null;
    pendingSeekPercent = null;
    currentTrackIndex.value = null;
    currentPlaylistIndex.value = null;
    playlistsHistory.value = [];
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

    // Computed
    currentTrack,
    canGoToPreviousPlaylist,
    canGoToNextPlaylist,

    // Constants
    PLAYBACK_CONTEXTS,

    // Playlist starters
    setAlbumId,
    setTrack,
    setUserPlaylist,
    setPlaylistFromTrackIds,

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
  };
});
