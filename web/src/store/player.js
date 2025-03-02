import { defineStore } from 'pinia';
import { computed, ref, watch } from 'vue';
import { Howl } from 'howler';
import { formatImageUrl, chooseAlbumCoverImageUrl } from '@/utils';
import { useRemoteStore } from './remote';

export const usePlayerStore = defineStore('player', () => {
  const remoteStore = useRemoteStore();

  /* PROPS */
  const MAX_PLAYLISTS_HISTORY = 20;
  const playlistsHistory = ref(null);
  const currentPlaylistIndex = ref(null);

  const currentTrackIndex = ref(null);
  const currentTrack = ref(null);
  const isPlaying = ref(false);
  const progressPercent = ref(0.0);
  const progressSec = ref(0);
  const volume = ref(0.5);
  const muted = ref(false);

  let sound = null;
  let pendingPercentSeek = null;

  const currentPlaylist = computed(() => {
    if (currentPlaylistIndex.value !== null) {
      return playlistsHistory.value[currentPlaylistIndex.value];
    }
    return null;
  });

  const canGoToPreviousPlaylist = computed(() => currentPlaylistIndex.value > 0);
  const canGoToNextPlaylist = computed(() => currentPlaylistIndex.value < (playlistsHistory.value.length - 1));
  /* PROPS */

  /* PERSISTENCE */
  const savedPlaylistsHistory = localStorage.getItem("playlistsHistory");
  if (savedPlaylistsHistory) {
    playlistsHistory.value = JSON.parse(savedPlaylistsHistory);
    if (playlistsHistory.value) {

      const savedCurrentPlaylistIndex = localStorage.getItem("currentPlaylistIndex") || playlistsHistory.value.length - 1;
      currentPlaylistIndex.value = Number.parseInt(savedCurrentPlaylistIndex);

      const loadedTrackIndex = localStorage.getItem("currentTrackIndex");
      if (loadedTrackIndex) {
        const indexValue = Number.parseInt(loadedTrackIndex);
        console.log("Loaded currenTrackIndex " + indexValue);
        if (Number.isInteger(indexValue) && !Number.isNaN(indexValue) && indexValue >= 0 && indexValue < currentPlaylist.value.tracks.length) {
          currentTrackIndex.value = indexValue;
          currentTrack.value = currentPlaylist.value.tracks[indexValue]
        }
      }
      const savedPercent = Number.parseFloat(localStorage.getItem("progressPercent"));
      console.log("loaded savedPercent " + savedPercent);
      if (!Number.isNaN(savedPercent) && savedPercent >= 0.0 && savedPercent <= 1.0) {
        pendingPercentSeek = savedPercent;
        progressPercent.value = savedPercent;
        console.log("seeking saved percent");
      }
      const savedSec = Number.parseFloat(localStorage.getItem("progressSec"));
      if (!Number.isNaN(savedSec)) {
        progressSec.value = savedSec;
      }
    }
  }
  watch(playlistsHistory, (newPlaylistsHistory) => localStorage.setItem("playlistsHistory", JSON.stringify(newPlaylistsHistory)))

  const savedMuted = localStorage.getItem("muted");
  if (savedMuted === 'true') {
    muted.value = true;
  }
  watch(muted, (newMuted) => localStorage.setItem('muted', newMuted));

  const savedVolume = localStorage.getItem('volume');
  if (savedVolume) {
    const parseVolume = Number.parseFloat(savedVolume);
    if (!Number.isNaN(parseFloat)) {
      volume.value = Math.max(0.0, Math.min(1.0, parseVolume));
    }
  }
  watch(volume, (newVolume) => localStorage.setItem("volume", newVolume));

  watch(currentTrackIndex, (newTrackIndex) => {
    if (Number.isInteger(newTrackIndex)) {
      localStorage.setItem('currentTrackIndex', newTrackIndex);
    }
  });

  function persistProgressPercent() {
    localStorage.setItem("progressPercent", progressPercent.value);
    lastSecProgressSaved = progressSec.value || 0;
    localStorage.setItem("progressSec", progressSec.value);
  }

  let lastSecProgressSaved = 0;
  watch(progressSec, (newSec) => {
    let diff = Math.abs(Math.round(newSec) - lastSecProgressSaved);
    if (diff > 4) {
      persistProgressPercent();
    }

  });
  /* PERSISTENCE */

  /* ACTIONS */
  const formatTrackUrl = (trackId) => "/v1/content/stream/" + trackId;

  const makePlaylistFromResolvedAlbumResponse = (response) => {
    const albumImageUrls = chooseAlbumCoverImageUrl(response.album);
    const allTracks = response.album.discs.flatMap(disc => disc.tracks).map((trackId) => {
      const track = response.tracks[trackId];
      const artistsIdsNames = track.artists_ids.map((artistId) => [artistId, response.artists[artistId].name]);
      return {
        id: trackId,
        url: formatTrackUrl(trackId),
        name: track.name,
        artists: artistsIdsNames,
        imageUrls: albumImageUrls,
        duration: track.duration,
        albumId: response.album.id,
      };
    });

    return {
      album: {
        name: response.album.name,
        id: response.album.id,
      },
      tracks: allTracks,
    }
  }

  const makePlaylistFromResolvedTracksAndPlaylist = (resolvedTracks, playlist) => {
    const tracks = resolvedTracks.map((resolvedTrackData) => {
      const track = Object.values(resolvedTrackData.tracks)[0];
      const artistsIdsNames = track.artists_ids.map((artistId) => [artistId, resolvedTrackData.artists[artistId].name]);
      return {
        id: track.id,
        url: formatTrackUrl(track.id),
        name: track.name,
        artists: artistsIdsNames,
        imageUrls: [formatImageUrl(track.image_id)],
        duration: track.duration,
        albumId: track.album_id,
      }
    });
    return {
      album: playlist,
      tracks: tracks,
    };
  }

  const makePlaylistFromTrack = (quakTrack) => {
    return {
      album: null,
      tracks: [
        {
          id: quakTrack.id,
          url: formatTrackUrl(quakTrack.id),
          name: quakTrack.name,
          artists: quakTrack.artists_ids_names,
          imageUrls: [formatImageUrl(quakTrack.image_id)],
          duration: quakTrack.duration,
          albumId: quakTrack.album_id,
        }
      ]
    };
  }

  const findTrackIndex = (album, discIndex, trackIndex) => {
    let previousDiscsTracks = 0;
    if (discIndex > 0) {
      for (let i = 0; i < discIndex; i++) {
        previousDiscsTracks += album.discs[i].tracks.length;
      }
      album.discs.map((disc) => disc.tracks.length).slice(0, discIndex - 1)
    }
    return trackIndex + previousDiscsTracks;
  }

  const setNewPlaylingPlaylist = (newPlaylist) => {
    pendingPercentSeek = null;
    let newHistory;

    if (playlistsHistory.value && currentPlaylistIndex.value !== null && currentPlaylistIndex.value < playlistsHistory.value.length - 1) {
      // If we're not at the end of history, remove all future playlists
      newHistory = [...playlistsHistory.value.slice(0, currentPlaylistIndex.value + 1), newPlaylist];
    } else {
      // Otherwise, just append normally
      newHistory = [...(playlistsHistory.value || []), newPlaylist];
    }

    if (newHistory.length > MAX_PLAYLISTS_HISTORY) {
      newHistory = newHistory.slice(newHistory.length - MAX_PLAYLISTS_HISTORY);
    }

    playlistsHistory.value = newHistory;
    currentPlaylistIndex.value = newHistory.length - 1;
  };

  const setResolvedAlbum = (data, discIndex, trackIndex) => {
    console.log("player.setResolvedAlbum() data:");
    console.log(data);
    pendingPercentSeek = null;
    if (!currentPlaylist.value || !currentPlaylist.value.album || currentPlaylist.value.album.id != data.album.id) {
      const albumPlaylist = makePlaylistFromResolvedAlbumResponse(data);
      setNewPlaylingPlaylist(albumPlaylist);
    }
    if (Number.isInteger(discIndex) && Number.isInteger(trackIndex)) {
      const desiredTrackIndex = findTrackIndex(data.album, discIndex, trackIndex);
      loadTrack(desiredTrackIndex);
    } else {
      loadTrack(0);
    }
    play();
  }

  const setAlbumId = async (albumId) => {
    try {
      const albumData = await remoteStore.fetchResolvedAlbum(albumId);
      if (albumData) {
        const albumPlaylist = makePlaylistFromResolvedAlbumResponse(albumData);
        setNewPlaylingPlaylist(albumPlaylist);
        loadTrack(0);
        play();
      }
    } catch (error) {
      console.error('Error setting album:', error);
    }
  }

  const setTrack = (newTrack) => {
    const trackPlaylist = makePlaylistFromTrack(newTrack)
    console.log("PlayerStore setTrack:");
    console.log(trackPlaylist);
    setNewPlaylingPlaylist(trackPlaylist);
    loadTrack(0);
    play();
  };

  const setPlaylist = async (newPlaylist) => {
    console.log("player setPlaylist:");
    console.log(newPlaylist);
    if (newPlaylist.tracks.length === 0) {
      return;
    }
    const resolvedTracks = newPlaylist.tracks.map(async (track) => {
      console.log("fetching track data for track:");
      console.log(track);
      const resolvedTrack = remoteStore.fetchResolvedTrack(track);
      console.log(resolvedTrack);
      return resolvedTrack;
    });
    const waitedTracks = await Promise.all(resolvedTracks);
    console.log("resolved tracks:");
    console.log(waitedTracks);
    const userPlaylistPlaylist = makePlaylistFromResolvedTracksAndPlaylist(waitedTracks, newPlaylist);
    setNewPlaylingPlaylist(userPlaylistPlaylist);
    loadTrack(0);
    play();
  }

  const loadTrack = (index) => {

    if (sound) {
      sound.unload();
    }

    currentTrackIndex.value = index;
    console.log("loadTrack() playlist:");
    console.log(currentPlaylist.value);
    const track = currentPlaylist.value.tracks[index]
    currentTrack.value = track;
    sound = new Howl({
      src: [track.url],
      html5: true,
      preload: true,
      volume: muted.value ? 0.0 : volume.value,
      pos: 10,
      autoplay: isPlaying.value,
      onend: () => skipNextTrack(),
      onplay: () => {
        requestUpdateProgressOnNewFrame();
        console.log("PlayerStore onplay()");
      },
      onload: () => {
        if (pendingPercentSeek) {
          requestAnimationFrame(() => {
            seekToPercentage(pendingPercentSeek);
            pendingPercentSeek = null;
            updateProgress();
          });
        }
        console.log("PlayerStore onLoad()");
      },
    });
    requestUpdateProgressOnNewFrame();
  }

  let lastUpdateMs = 0;

  const relaxedUpdateProgress = () => {
    updateProgress(true)
  }
  const requestUpdateProgressOnNewFrame = () => {
    requestAnimationFrame(relaxedUpdateProgress);
  }
  const updateProgress = (relaxed) => {
    if (relaxed) {
      if (Date.now() - lastUpdateMs < 300) {
        requestUpdateProgressOnNewFrame();
        return;
      }
    }
    if (sound) {
      const currentTime = sound.seek();
      const duration = sound.duration();
      progressPercent.value = (currentTime / duration);
      progressSec.value = currentTime;
      if (sound.playing()) {
        requestUpdateProgressOnNewFrame();
      }
      lastUpdateMs = Date.now();
    }
  }

  const skipNextTrack = () => {
    let nextIndex = currentTrackIndex.value + 1;
    if (nextIndex >= currentPlaylist.value.tracks.length) {
      if (sound) {
        sound.unload();
      }
      isPlaying.value = false;
      progressPercent.value = 0.0;
      progressSec.value = 0;
      return;
    }
    loadTrack(nextIndex);
    if (isPlaying.value) {
      play();
    }
  }

  const skipPreviousTrack = () => {
    let previousIndex = currentTrackIndex.value - 1;
    if (previousIndex < 0) {
      if (sound) {
        sound.unload();
      }
      isPlaying.value = false;
      progressPercent.value = 0.0;
      progressSec.value = 0;
      return;
    }
    loadTrack(previousIndex);
    if (isPlaying.value) {
      play();
    }
  }

  const play = () => {
    if (!sound) {
      loadTrack(currentTrackIndex.value);
    }
    sound.play();
    isPlaying.value = true
  }

  const pause = () => {
    if (sound) {
      sound.pause();
    }
    isPlaying.value = false
  }

  const playPause = () => {
    console.log("PlayerStore playPause() current value: " + isPlaying.value);
    const newValue = !isPlaying.value;
    if (newValue) {
      play();
    } else {
      pause();
    }
  };
  const setIsPlaying = (newIsPlaying) => {
    if (isPlaying.value != newIsPlaying) {
      isPlaying.value = newIsPlaying;
      if (newIsPlaying) {
        play();
      } else {
        pause();
      }
    }
  };

  const seekToPercentage = (percentage) => {
    if (sound) {
      const duration = sound.duration();
      const seekTime = (duration * percentage);
      const play = sound.playing();
      sound.seek(seekTime);
      if (play) {
        sound.play();
      }
      updateProgress();
      requestUpdateProgressOnNewFrame();
      persistProgressPercent();
    }
  };

  const forward10Sec = () => {
    if (sound) {
      sound.seek(sound.seek() + 10);
      updateProgress();
      requestUpdateProgressOnNewFrame();
    }
  }

  const rewind10Sec = () => {
    if (sound) {
      sound.seek(sound.seek() - 10);
      updateProgress();
      requestUpdateProgressOnNewFrame();
    }
  }

  const stop = () => {
    if (sound) {
      sound.unload();
    }
    sound = null;
    isPlaying.value = false;
    progressPercent.value = 0.0;
    progressSec.value = 0;
    currentTrack.value = null;
    pendingPercentSeek = null;
    currentTrackIndex.value = null;
    currentPlaylistIndex.value = null
    playlistsHistory.value = [];
  }

  const setVolume = (newVolume) => {
    if (sound) {
      sound.volume(newVolume);
    }
    volume.value = newVolume;
  }

  const setMuted = (newMuted) => {
    if (sound) {
      sound.volume(newMuted ? 0.0 : volume.value);
    }
    muted.value = newMuted;
  }

  const loadTrackIndex = (index) => {
    pendingPercentSeek = null;
    if (currentPlaylist.value.tracks.length && index >= 0 && index < currentPlaylist.value.tracks.length) {
      currentTrackIndex.value = index;
      loadTrack(index);
      if (isPlaying.value) {
        play();
      }
    }
  }

  const goToPreviousPlaylist = () => {
    if (canGoToPreviousPlaylist.value) {
      currentPlaylistIndex.value -= 1;
      loadTrack(0);
      if (isPlaying.value) {
        play();
      }
    }
  }
  const goToNextPlaylist = () => {
    if (canGoToNextPlaylist.value) {
      currentPlaylistIndex.value += 1;
      loadTrack(0);
      if (isPlaying.value) {
        play();
      }
    }
  }
  /* ACTIONS */

  return {
    currentPlaylist,
    currentTrackIndex,
    currentTrack,
    isPlaying,
    progressPercent,
    progressSec,
    volume,
    muted,
    canGoToPreviousPlaylist,
    canGoToNextPlaylist,
    setTrack,
    setPlaylist,
    setResolvedAlbum,
    setAlbumId,
    seekToPercentage,
    setIsPlaying,
    playPause,
    skipPreviousTrack,
    skipNextTrack,
    forward10Sec,
    rewind10Sec,
    stop,
    setVolume,
    setMuted,
    loadTrackIndex,
    goToPreviousPlaylist,
    goToNextPlaylist,
  };
});
