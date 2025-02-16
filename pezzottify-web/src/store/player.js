import { defineStore } from 'pinia';
import { ref } from 'vue';
import { Howl } from 'howler';
import { formatImageUrl, chooseAlbumCoverImageUrl } from '@/utils';
import axios from 'axios';

export const usePlayerStore = defineStore('player', () => {
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

  const formatTrackUrl = (trackId) => "/v1/content/stream/" + trackId;

  const makePlaylistFromResolvedAlbumResponse = (response) => {
    const albumImageUrls = chooseAlbumCoverImageUrl(response.album);
    const allTracks = response.album.discs.flatMap(disc => disc.tracks).map((trackId) => {
      const track = response.tracks[trackId];
      console.log("track:");
      console.log(track);
      const artistsIdsNames = track.artists_ids.map((artistId) => [artistId, response.artists[artistId].name]);
      return {
        id: trackId,
        url: formatTrackUrl(trackId),
        name: track.name,
        artists: artistsIdsNames,
        imageUrls: albumImageUrls,
        duration: track.duration,
      };
    });

    return {
      album: {
        name: response.album.name,
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
        }
      ]
    };
  }

  const setAlbum = async (albumId) => {
    try {
      const response = await axios.get(`/v1/content/album/${albumId}/resolved`);
      console.log("setAlbum resolved response:");
      console.log(response.data);
      const albumPlaylist = makePlaylistFromResolvedAlbumResponse(response.data);
      console.log("setAlbum " + albumId + " playlist =>");
      console.log(albumPlaylist);
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
    const track = playlist.value.tracks[index]
    currentTrack.value = track;
    sound = new Howl({
      src: [track.url],
      html5: true,
      volume: muted.value ? 0.0 : volume.value,
      onend: () => skipNextTrack(),
      onplay: () => {
        console.log("PlayerStore onplay()");
        requestAnimationFrame(updateProgress);
      }
    });
    requestAnimationFrame(updateProgress);
  }

  const updateProgress = () => {
    if (sound) {
      const currentTime = sound.seek();
      const duration = sound.duration();
      progressPercent.value = (currentTime / duration);
      progressSec.value = currentTime;
      if (sound.playing()) {
        requestAnimationFrame(updateProgress);
      }
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
      requestAnimationFrame(updateProgress);
    }
  };

  const forward10Sec = () => {
    if (sound) {
      sound.seek(sound.seek() + 10);
      updateProgress();
      requestAnimationFrame(updateProgress);
    }
  }

  const rewind10Sec = () => {
    if (sound) {
      sound.seek(sound.seek() - 10);
      updateProgress();
      requestAnimationFrame(updateProgress);
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
    playlist.value = null;
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
    setAlbum,
    playPause,
    skipPreviousTrack,
    skipNextTrack,
    forward10Sec,
    rewind10Sec,
    stop,
    setVolume,
    setMuted,
  };
});
