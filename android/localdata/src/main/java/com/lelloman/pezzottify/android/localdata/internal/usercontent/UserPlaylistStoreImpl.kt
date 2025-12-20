package com.lelloman.pezzottify.android.localdata.internal.usercontent

import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
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

    override fun getPendingSyncPlaylists(): Flow<List<UserPlaylist>> =
        playlistDao.getPendingSyncItems().map { entities -> entities.map { it.toDomain() } }

    override suspend fun replaceAllPlaylists(playlists: List<UserPlaylist>) {
        playlistDao.replaceAll(playlists.map { it.toEntity() })
    }

    override suspend fun createOrUpdatePlaylist(
        id: String,
        name: String,
        trackIds: List<String>,
        syncStatus: PlaylistSyncStatus,
    ) {
        playlistDao.upsert(
            PlaylistEntity(
                id = id,
                name = name,
                trackIds = trackIds,
                syncStatus = syncStatus,
            )
        )
    }

    override suspend fun deletePlaylist(playlistId: String) {
        playlistDao.deleteById(playlistId)
    }

    override suspend fun markPlaylistForDeletion(playlistId: String) {
        playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingDelete)
    }

    override suspend fun updatePlaylistName(playlistId: String, name: String) {
        val playlist = playlistDao.getByIdOnce(playlistId) ?: return
        playlistDao.updateName(playlistId, name)
        // If already synced, mark as needing update
        if (playlist.syncStatus == PlaylistSyncStatus.Synced) {
            playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate)
        }
    }

    override suspend fun updatePlaylistTracks(playlistId: String, trackIds: List<String>) {
        val playlist = playlistDao.getByIdOnce(playlistId) ?: return
        playlistDao.updateTrackIds(playlistId, trackIds)
        // If already synced, mark as needing update
        if (playlist.syncStatus == PlaylistSyncStatus.Synced) {
            playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate)
        }
    }

    override suspend fun addTrackToPlaylist(playlistId: String, trackId: String) {
        val playlist = playlistDao.getByIdOnce(playlistId) ?: return
        if (trackId !in playlist.trackIds) {
            playlistDao.updateTrackIds(playlistId, playlist.trackIds + trackId)
            // If already synced, mark as needing update
            if (playlist.syncStatus == PlaylistSyncStatus.Synced) {
                playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate)
            }
        }
    }

    override suspend fun addTracksToPlaylist(playlistId: String, trackIds: List<String>) {
        val playlist = playlistDao.getByIdOnce(playlistId) ?: return
        val existingIds = playlist.trackIds.toSet()
        val newTracks = trackIds.filter { it !in existingIds }
        if (newTracks.isNotEmpty()) {
            playlistDao.updateTrackIds(playlistId, playlist.trackIds + newTracks)
            // If already synced, mark as needing update
            if (playlist.syncStatus == PlaylistSyncStatus.Synced) {
                playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate)
            }
        }
    }

    override suspend fun removeTrackFromPlaylist(playlistId: String, trackId: String) {
        val playlist = playlistDao.getByIdOnce(playlistId) ?: return
        if (trackId in playlist.trackIds) {
            playlistDao.updateTrackIds(playlistId, playlist.trackIds - trackId)
            // If already synced, mark as needing update
            if (playlist.syncStatus == PlaylistSyncStatus.Synced) {
                playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate)
            }
        }
    }

    override suspend fun updateSyncStatus(playlistId: String, syncStatus: PlaylistSyncStatus) {
        playlistDao.updateSyncStatus(playlistId, syncStatus)
    }

    override suspend fun deleteAll() {
        playlistDao.deleteAll()
    }
}

private fun UserPlaylist.toEntity() = PlaylistEntity(
    id = id,
    name = name,
    trackIds = trackIds,
    syncStatus = syncStatus,
)
