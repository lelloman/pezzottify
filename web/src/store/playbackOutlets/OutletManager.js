/**
 * OutletManager - Orchestrates playback outlets
 *
 * Manages switching between local and remote outlets,
 * delegating commands to the active outlet.
 */

import { LocalOutlet } from "./LocalOutlet";
import { RemoteOutlet } from "./RemoteOutlet";

export class OutletManager {
  constructor(callbacks) {
    this.callbacks = callbacks;
    this.localOutlet = new LocalOutlet(callbacks);
    this.remoteOutlet = null; // Created when devices store is available
    this.activeOutlet = this.localOutlet;
    this.outletType = "local"; // 'local' or 'remote'
  }

  /**
   * Set the devices store for remote outlet
   */
  setDevicesStore(devicesStore) {
    this.remoteOutlet = new RemoteOutlet(this.callbacks, devicesStore);
  }

  /**
   * Get the current outlet type
   */
  getOutletType() {
    return this.outletType;
  }

  /**
   * Check if local output is active
   */
  isLocalOutput() {
    return this.outletType === "local";
  }

  /**
   * Switch to local outlet
   */
  switchToLocal(stateSnapshot) {
    if (this.outletType === "local") return;

    // Release remote outlet
    if (this.remoteOutlet) {
      this.remoteOutlet.releaseControl();
    }

    // Assume control with local
    this.localOutlet.assumeControl(stateSnapshot);
    this.activeOutlet = this.localOutlet;
    this.outletType = "local";
  }

  /**
   * Switch to remote outlet
   */
  switchToRemote() {
    if (this.outletType === "remote") return;
    if (!this.remoteOutlet) {
      console.warn("[OutletManager] Remote outlet not initialized");
      return;
    }

    // Release local outlet
    this.localOutlet.releaseControl();

    // Assume control with remote
    this.remoteOutlet.assumeControl();
    this.activeOutlet = this.remoteOutlet;
    this.outletType = "remote";
  }

  /**
   * Update remote state (from WebSocket)
   */
  updateRemoteState(state) {
    if (this.remoteOutlet) {
      this.remoteOutlet.updateRemoteState(state);
    }
  }

  // ============================================
  // Playback commands - delegated to active outlet
  // ============================================

  loadTrack(trackId, autoPlay = false, seekPercent = null) {
    // Only local outlet loads tracks
    this.localOutlet.loadTrack(trackId, autoPlay, seekPercent);
  }

  play() {
    this.activeOutlet.play();
  }

  pause() {
    this.activeOutlet.pause();
  }

  seekTo(seconds) {
    this.activeOutlet.seekTo(seconds);
  }

  seekToPercentage(percentage) {
    this.activeOutlet.seekToPercentage(percentage);
  }

  setVolume(vol) {
    this.activeOutlet.setVolume(vol);
  }

  setMuted(muted, volume) {
    if (this.outletType === "local") {
      this.localOutlet.setMuted(muted, volume);
    } else {
      this.remoteOutlet.setMuted(muted);
    }
  }

  stop() {
    this.activeOutlet.stop();
  }

  skipNext() {
    if (this.outletType === "remote") {
      this.remoteOutlet.skipNext();
    }
    // Local skip is handled by playback store
  }

  skipPrevious() {
    if (this.outletType === "remote") {
      this.remoteOutlet.skipPrevious();
    }
    // Local skip is handled by playback store
  }

  forward10Sec() {
    if (this.outletType === "remote") {
      this.remoteOutlet.forward10Sec();
    } else {
      const pos = this.localOutlet.getPosition();
      this.localOutlet.seekTo(pos + 10);
    }
  }

  rewind10Sec() {
    if (this.outletType === "remote") {
      this.remoteOutlet.rewind10Sec();
    } else {
      const pos = this.localOutlet.getPosition();
      this.localOutlet.seekTo(Math.max(0, pos - 10));
    }
  }

  // ============================================
  // State getters
  // ============================================

  hasLoadedSound() {
    return this.localOutlet.hasLoadedSound();
  }

  getPosition() {
    return this.activeOutlet.getPosition();
  }

  getDuration() {
    return this.activeOutlet.getDuration();
  }

  isPlaying() {
    return this.activeOutlet.isPlaying();
  }
}
