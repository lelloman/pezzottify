/**
 * RemoteOutlet - Remote playback via WebSocket
 *
 * Implements the Outlet interface for remote playback control.
 * Sends commands via WebSocket and interpolates position between updates.
 */

export class RemoteOutlet {
  constructor(callbacks, devicesStore) {
    this.callbacks = callbacks;
    this.devicesStore = devicesStore;
    this.isActive = false;

    // Remote state from server
    this.remoteState = null;

    // Position interpolation
    this.interpolatedPosition = 0;
    this.interpolationFrame = null;
  }

  /**
   * Update remote state from server
   */
  updateRemoteState(state) {
    this.remoteState = state;

    // If we're the active outlet, notify the playback store
    if (this.isActive && state) {
      this.callbacks.onRemoteStateUpdate(state);
    }
  }

  /**
   * Start or resume playback
   */
  play() {
    this.devicesStore.sendCommand("play");
  }

  /**
   * Pause playback
   */
  pause() {
    this.devicesStore.sendCommand("pause");
  }

  /**
   * Seek to a position in seconds
   */
  seekTo(seconds) {
    this.devicesStore.sendCommand("seek", { position: seconds });
  }

  /**
   * Seek to a percentage (0.0 - 1.0)
   */
  seekToPercentage(percentage) {
    const duration = this.remoteState?.current_track?.duration || 0;
    if (duration > 0) {
      this.seekTo(percentage * duration);
    }
  }

  /**
   * Set volume (0.0 - 1.0)
   */
  setVolume(vol) {
    this.devicesStore.sendCommand("setVolume", { volume: vol });
  }

  /**
   * Set muted state
   */
  setMuted(muted) {
    this.devicesStore.sendCommand("setMuted", { muted });
  }

  /**
   * Stop playback (not really applicable for remote)
   */
  stop() {
    // Remote doesn't support stop - just pause
    this.pause();
  }

  /**
   * Skip to next track
   */
  skipNext() {
    this.devicesStore.sendCommand("next");
  }

  /**
   * Skip to previous track
   */
  skipPrevious() {
    this.devicesStore.sendCommand("prev");
  }

  /**
   * Forward 10 seconds
   */
  forward10Sec() {
    const currentPos = this.interpolatedPosition || 0;
    this.seekTo(currentPos + 10);
  }

  /**
   * Rewind 10 seconds
   */
  rewind10Sec() {
    const currentPos = this.interpolatedPosition || 0;
    this.seekTo(Math.max(0, currentPos - 10));
  }

  /**
   * Load a track (not applicable for remote - handled by audio device)
   */
  loadTrack() {
    // Remote outlet doesn't load tracks - the audio device does
  }

  /**
   * Called when this outlet becomes active
   */
  assumeControl() {
    this.isActive = true;
    this.startInterpolation();
  }

  /**
   * Called when this outlet is no longer active
   */
  releaseControl() {
    this.isActive = false;
    this.stopInterpolation();
  }

  /**
   * Get current interpolated position in seconds
   */
  getPosition() {
    return this.interpolatedPosition;
  }

  /**
   * Get duration from remote state
   */
  getDuration() {
    return this.remoteState?.current_track?.duration || 0;
  }

  /**
   * Check if currently playing
   */
  isPlaying() {
    return this.remoteState?.is_playing || false;
  }

  /**
   * Start position interpolation
   */
  startInterpolation() {
    if (this.interpolationFrame) return;

    const tick = () => {
      if (this.remoteState?.is_playing) {
        const elapsed = (Date.now() - this.remoteState.timestamp) / 1000;
        this.interpolatedPosition = this.remoteState.position + elapsed;
      } else if (this.remoteState) {
        this.interpolatedPosition = this.remoteState.position;
      }

      // Notify callback of progress
      if (this.isActive) {
        const duration = this.getDuration();
        const percent = duration > 0 ? this.interpolatedPosition / duration : 0;
        this.callbacks.onProgressUpdate(this.interpolatedPosition, percent);
      }

      this.interpolationFrame = requestAnimationFrame(tick);
    };
    tick();
  }

  /**
   * Stop position interpolation
   */
  stopInterpolation() {
    if (this.interpolationFrame) {
      cancelAnimationFrame(this.interpolationFrame);
      this.interpolationFrame = null;
    }
  }
}
