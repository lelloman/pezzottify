import { defineStore } from 'pinia';
import { ref } from 'vue';
import { Howl } from 'howler';
import { formatImageUrl } from '@/utils';

export const usePlayerStore = defineStore('player', () => {
  const playlist = ref([]);
  const currentTrackIndex = ref(null);
  const currentTrack = ref(null);
  const isPlaying = ref(false);
  const progressPercent = ref(0.0);
  const progressSec = ref(0);

  let sound = null;

  const setTrack = (newTrack) => {
    const track = {
      id: newTrack.id,
      url: "/v1/content/stream/" + newTrack.id,
      name: newTrack.name,
      artist: newTrack.artists_names.join(", "),
      imageUrl: formatImageUrl(newTrack.image_id),
      duration: newTrack.duration,
    }
    console.log("PlayerStore setTrack:");
    console.log(track);
    playlist.value = [track];
    loadTrack(0);
    play();
  };

  const loadTrack = (index) => {
    if (sound) {
      sound.unload();
    }

    currentTrackIndex.value = index;
    const track = playlist.value[index]
    currentTrack.value = track;
    sound = new Howl({
      src: [track.url],
      html5: true,
      onend: () => nextTrack(),
      onplay: () => {
        console.log("PlayerStore onplay()");
        requestAnimationFrame(updateProgress);
      }
    });
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

  const nextTrack = () => {
    let nextIndex = currentTrackIndex.value + 1;
    if (nextIndex >= playlist.value.length) {
      return;
    }
    loadTrack(nextIndex);
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
    }
  };

  return {
    playlist,
    currentTrackIndex,
    currentTrack,
    isPlaying,
    progressPercent,
    progressSec,
    seekToPercentage,
    setIsPlaying,
    setTrack,
    playPause,
  };
});
