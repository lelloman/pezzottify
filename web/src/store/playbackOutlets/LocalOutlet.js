/**
 * LocalOutlet - Howler.js audio playback wrapper
 *
 * Implements the Outlet interface for local audio playback.
 * Manages Howler.js instances and progress tracking.
 */

import { Howl } from "howler";

export class LocalOutlet {
  constructor(callbacks) {
    // Callbacks to notify the playback store of state changes
    this.callbacks = callbacks;
    this.sound = null;
    this.lastUpdateMs = 0;
    this.isActive = false;
  }

  /**
   * Format track URL for streaming
   */
  formatTrackUrl(trackId) {
    return "/v1/content/stream/" + trackId;
  }

  /**
   * Load a track and optionally start playback
   */
  loadTrack(trackId, autoPlay = false, seekPercent = null) {
    if (this.sound) {
      this.sound.unload();
    }

    this.sound = new Howl({
      src: [this.formatTrackUrl(trackId)],
      html5: true,
      preload: true,
      volume: this.callbacks.getVolume(),
      autoplay: autoPlay,
      onend: () => this.callbacks.onTrackEnd(),
      onplay: () => {
        this.requestUpdateProgressOnNewFrame();
        this.callbacks.onPlayStateChange(true);
      },
      onpause: () => {
        this.callbacks.onPlayStateChange(false);
      },
      onload: () => {
        if (seekPercent !== null && seekPercent > 0) {
          requestAnimationFrame(() => {
            this.seekToPercentage(seekPercent);
            this.updateProgress();
          });
        }
        this.callbacks.onTrackLoaded(this.sound.duration());
      },
    });
  }

  /**
   * Start or resume playback
   */
  play() {
    if (this.sound) {
      this.sound.play();
    }
  }

  /**
   * Pause playback
   */
  pause() {
    if (this.sound) {
      this.sound.pause();
    }
  }

  /**
   * Seek to a position in seconds
   */
  seekTo(seconds) {
    if (this.sound) {
      const wasPlaying = this.sound.playing();
      this.sound.seek(seconds);
      if (wasPlaying) {
        this.sound.play();
      }
      this.updateProgress();
      this.requestUpdateProgressOnNewFrame();
    }
  }

  /**
   * Seek to a percentage (0.0 - 1.0)
   */
  seekToPercentage(percentage) {
    if (this.sound) {
      const duration = this.sound.duration();
      const seekTime = duration * percentage;
      this.seekTo(seekTime);
    }
  }

  /**
   * Set volume (0.0 - 1.0)
   */
  setVolume(vol) {
    if (this.sound) {
      this.sound.volume(vol);
    }
  }

  /**
   * Set muted state
   */
  setMuted(muted, volume) {
    if (this.sound) {
      this.sound.volume(muted ? 0.0 : volume);
    }
  }

  /**
   * Stop playback and unload
   */
  stop() {
    if (this.sound) {
      this.sound.unload();
      this.sound = null;
    }
  }

  /**
   * Called when this outlet becomes active
   */
  assumeControl(stateSnapshot) {
    this.isActive = true;
    // If there's a track loaded, apply state
    if (stateSnapshot.trackId && this.sound) {
      this.setVolume(stateSnapshot.muted ? 0 : stateSnapshot.volume);
      if (stateSnapshot.position > 0) {
        this.seekTo(stateSnapshot.position);
      }
      if (stateSnapshot.isPlaying) {
        this.play();
      }
    }
  }

  /**
   * Called when this outlet is no longer active
   */
  releaseControl() {
    this.isActive = false;
    this.pause();
  }

  /**
   * Get current position in seconds
   */
  getPosition() {
    if (this.sound) {
      return this.sound.seek() || 0;
    }
    return 0;
  }

  /**
   * Get duration in seconds
   */
  getDuration() {
    if (this.sound) {
      return this.sound.duration() || 0;
    }
    return 0;
  }

  /**
   * Check if currently playing
   */
  isPlaying() {
    return this.sound?.playing() || false;
  }

  /**
   * Request progress update on next frame (throttled)
   */
  requestUpdateProgressOnNewFrame() {
    requestAnimationFrame(() => this.relaxedUpdateProgress());
  }

  /**
   * Throttled progress update
   */
  relaxedUpdateProgress() {
    if (Date.now() - this.lastUpdateMs < 300) {
      this.requestUpdateProgressOnNewFrame();
      return;
    }
    this.updateProgress();
  }

  /**
   * Update progress and notify callback
   */
  updateProgress() {
    if (this.sound) {
      const currentTime = this.sound.seek();
      const duration = this.sound.duration();
      const percent = duration > 0 ? currentTime / duration : 0;

      this.callbacks.onProgressUpdate(currentTime, percent);

      if (this.sound.playing()) {
        this.requestUpdateProgressOnNewFrame();
      }
      this.lastUpdateMs = Date.now();
    }
  }
}
