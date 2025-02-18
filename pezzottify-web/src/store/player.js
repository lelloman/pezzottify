import { defineStore } from 'pinia';
import { ref, watch } from 'vue';
import { Howl } from 'howler';
import { formatImageUrl, chooseAlbumCoverImageUrl } from '@/utils';
import axios from 'axios';

export const usePlayerStore = defineStore('player', () => {

  /* PROPS */
  const playlist = ref({
    album: null,
    tracks: [],
  });
  const currentTrackIndex = ref(null);
  const currentTrack = ref(null);
  const isPlaying = ref(false);
  const progressPercent = ref(0.0);
  const progressSec = ref(0);
  const volume = ref(0.5);
  const muted = ref(false);

  let sound = null;
  let pendingPercentSeek = null;
  /* PROPS */

  /* PERSISTENCE */
  const savedPlaylist = localStorage.getItem("playlist");
  if (savedPlaylist) {
    playlist.value = JSON.parse(savedPlaylist);
    const loadedTrackIndex = localStorage.getItem("currentTrackIndex");
    if (loadedTrackIndex) {
      const indexValue = Number.parseInt(loadedTrackIndex);
      console.log("Loaded currenTrackIndex " + indexValue);
      if (!Number.isNaN(indexValue) && indexValue >= 0 && indexValue < playlist.value.tracks.length) {
        currentTrackIndex.value = indexValue;
        currentTrack.value = playlist.value.tracks[indexValue]
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
  watch(playlist, (newPlaylist) => localStorage.setItem("playlist", JSON.stringify(newPlaylist)))

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

  const setResolvedAlbum = (data, discIndex, trackIndex) => {
    console.log("player.setResolvedAlbum() data:");
    console.log(data);
    if (!playlist.value.album || playlist.value.album.id != data.album.id) {
      const albumPlaylist = makePlaylistFromResolvedAlbumResponse(data);
      playlist.value = albumPlaylist;
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
      const response = await axios.get(`/v1/content/album/${albumId}/resolved`);
      const albumPlaylist = makePlaylistFromResolvedAlbumResponse(response.data);
      playlist.value = albumPlaylist;
      loadTrack(0);
      play();
    } catch (error) {
      console.error('Error fetching data:', error);
    }
  }

  const setTrack = (newTrack) => {
    const trackPlaylist = makePlaylistFromTrack(newTrack)
    console.log("PlayerStore setTrack:");
    console.log(trackPlaylist);
    playlist.value = trackPlaylist;
    loadTrack(0);
    play();
  };

  const loadTrack = (index) => {

    if (sound) {
      sound.unload();
    }

    currentTrackIndex.value = index;
    console.log("loadTrack() playlist:");
    console.log(playlist);
    const track = playlist.value.tracks[index]
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
    if (nextIndex >= playlist.value.tracks.length) {
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
    playlist.value = { album: null, tracks: [] };
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
    if (playlist.value.tracks.length && index >= 0 && index < playlist.value.tracks.length) {
      currentTrackIndex.value = index;
      loadTrack(index);
      if (isPlaying.value) {
        play();
      }
    }
  }
  /* ACTIONS */

  return {
    playlist,
    currentTrackIndex,
    currentTrack,
    isPlaying,
    progressPercent,
    progressSec,
    volume,
    muted,
    seekToPercentage,
    setIsPlaying,
    setTrack,
    setAlbumId,
    setResolvedAlbum,
    playPause,
    skipPreviousTrack,
    skipNextTrack,
    forward10Sec,
    rewind10Sec,
    stop,
    setVolume,
    setMuted,
    loadTrackIndex,
  };
});
