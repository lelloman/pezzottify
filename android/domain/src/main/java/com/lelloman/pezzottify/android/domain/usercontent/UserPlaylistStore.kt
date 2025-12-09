package com.lelloman.pezzottify.android.domain.usercontent

import kotlinx.coroutines.flow.Flow

interface UserPlaylistStore {

    fun getPlaylists(): Flow<List<UserPlaylist>>

    fun getPlaylist(playlistId: String): Flow<UserPlaylist?>

    suspend fun replaceAllPlaylists(playlists: List<UserPlaylist>)

    suspend fun createOrUpdatePlaylist(id: String, name: String, trackIds: List<String>)

    suspend fun deletePlaylist(playlistId: String)

    suspend fun updatePlaylistName(playlistId: String, name: String)

    suspend fun updatePlaylistTracks(playlistId: String, trackIds: List<String>)

    suspend fun deleteAll()
}
