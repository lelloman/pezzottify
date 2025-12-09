package com.lelloman.pezzottify.android.localdata.internal.usercontent

import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylist
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.PlaylistEntity
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.toDomain
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

internal class UserPlaylistStoreImpl(
    private val playlistDao: PlaylistDao,
) : UserPlaylistStore {

    override fun getPlaylists(): Flow<List<UserPlaylist>> =
        playlistDao.getAll().map { entities -> entities.map { it.toDomain() } }

    override fun getPlaylist(playlistId: String): Flow<UserPlaylist?> =
        playlistDao.getById(playlistId).map { it?.toDomain() }

    override suspend fun replaceAllPlaylists(playlists: List<UserPlaylist>) {
        playlistDao.replaceAll(playlists.map { it.toEntity() })
    }

    override suspend fun createOrUpdatePlaylist(id: String, name: String, trackIds: List<String>) {
        playlistDao.upsert(
            PlaylistEntity(
                id = id,
                name = name,
                trackIds = trackIds,
            )
        )
    }

    override suspend fun deletePlaylist(playlistId: String) {
        playlistDao.deleteById(playlistId)
    }

    override suspend fun updatePlaylistName(playlistId: String, name: String) {
        playlistDao.updateName(playlistId, name)
    }

    override suspend fun updatePlaylistTracks(playlistId: String, trackIds: List<String>) {
        playlistDao.updateTrackIds(playlistId, trackIds)
    }

    override suspend fun deleteAll() {
        playlistDao.deleteAll()
    }
}

private fun UserPlaylist.toEntity() = PlaylistEntity(
    id = id,
    name = name,
    trackIds = trackIds,
)
