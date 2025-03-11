package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import kotlinx.coroutines.flow.StateFlow

interface Player : AppInitializer{

    val playbackPlaylist: StateFlow<PlaybackPlaylist?>
    val volumeState: StateFlow<VolumeState>
    val isPlaying: StateFlow<Boolean>
    val canGoToPreviousPlaylist: StateFlow<Boolean>
    val canGoToNextPlaylist: StateFlow<Boolean>

    fun loadAlbum(albumId: String)
    fun loadUserPlaylist(userPlaylistId: String)
    fun loadTrack(trackId: String)

    fun togglePlayPause()
    fun seekToPercentage(percentage: Float)
    fun setIsPlaying(isPlaying: Boolean)
    fun forward10Sec()
    fun rewind10Sec()
    fun stop()
    fun setVolume(volume: Float)
    fun setMuted(isMuted: Boolean)
    fun loadTrackIndex(index: Int)
    fun goToPreviousPlaylist()
    fun goToNextPlaylist()
    fun moveTrack(fromIndex: Int, toIndex: Int)
    fun addTracksToPlaylist(tracksIds: List<String>)
    fun removeTrackFromPlaylist(trackId: String)

    fun skipToNextTrack()
    fun skipToPreviousTrack()
}
