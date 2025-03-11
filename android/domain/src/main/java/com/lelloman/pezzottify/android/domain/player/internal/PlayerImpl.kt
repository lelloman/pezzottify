package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackState
import com.lelloman.pezzottify.android.domain.player.Player
import kotlinx.coroutines.flow.StateFlow

internal class PlayerImpl : Player {

    override val currentPlayback: StateFlow<PlaybackPlaylist?>
        get() = TODO("Not yet implemented")
    override val playbackState: StateFlow<PlaybackState?>
        get() = TODO("Not yet implemented")
    override val canGoToPreviousPlaylist: StateFlow<Boolean>
        get() = TODO("Not yet implemented")
    override val canGoToNextPlaylist: StateFlow<Boolean>
        get() = TODO("Not yet implemented")

    override fun loadAlbum(albumId: String) {
        TODO("Not yet implemented")
    }

    override fun loadUserPlaylist(userPlaylistId: String) {
        TODO("Not yet implemented")
    }

    override fun loadTrack(trackId: String) {
        TODO("Not yet implemented")
    }

    override fun togglePlayPause() {
        TODO("Not yet implemented")
    }

    override fun seekToPercentage(percentage: Float) {
        TODO("Not yet implemented")
    }

    override fun setIsPlaying(isPlaying: Boolean) {
        TODO("Not yet implemented")
    }

    override fun forward10Sec() {
        TODO("Not yet implemented")
    }

    override fun rewind10Sec() {
        TODO("Not yet implemented")
    }

    override fun stop() {
        TODO("Not yet implemented")
    }

    override fun setVolume(volume: Float) {
        TODO("Not yet implemented")
    }

    override fun setMuted(isMuted: Boolean) {
        TODO("Not yet implemented")
    }

    override fun loadTrackIndex(index: Int) {
        TODO("Not yet implemented")
    }

    override fun goToPreviousPlaylist() {
        TODO("Not yet implemented")
    }

    override fun goToNextPlaylist() {
        TODO("Not yet implemented")
    }

    override fun moveTrack(fromIndex: Int, toIndex: Int) {
        TODO("Not yet implemented")
    }

    override fun addTracksToPlaylist(tracksIds: List<String>) {
        TODO("Not yet implemented")
    }

    override fun removeTrackFromPlaylist(trackId: String) {
        TODO("Not yet implemented")
    }

    override fun skipToNextTrack() {
        TODO("Not yet implemented")
    }

    override fun skipToPreviousTrack() {
        TODO("Not yet implemented")
    }
}