package com.lelloman.pezzottify.android.localdata.internal.usercontent

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.PlaylistEntity
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.mockk
import io.mockk.slot
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class UserPlaylistStoreImplTest {

    private lateinit var playlistDao: PlaylistDao
    private lateinit var store: UserPlaylistStoreImpl

    @Before
    fun setup() {
        playlistDao = mockk(relaxed = true)
        store = UserPlaylistStoreImpl(playlistDao)
    }

    // ================================================================================
    // updatePlaylistTracks tests
    // ================================================================================

    @Test
    fun `updatePlaylistTracks with fromServer=false marks synced playlist as PendingUpdate`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When updating tracks with fromServer=false (local change)
        store.updatePlaylistTracks(playlistId, listOf("track-1", "track-2"), fromServer = false)

        // Then it should mark as PendingUpdate
        coVerify { playlistDao.updateTrackIds(playlistId, listOf("track-1", "track-2")) }
        coVerify { playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate) }
    }

    @Test
    fun `updatePlaylistTracks with fromServer=true does NOT mark playlist as PendingUpdate`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When updating tracks with fromServer=true (sync event)
        store.updatePlaylistTracks(playlistId, listOf("track-1", "track-2"), fromServer = true)

        // Then it should update tracks but NOT change sync status
        coVerify { playlistDao.updateTrackIds(playlistId, listOf("track-1", "track-2")) }
        coVerify(exactly = 0) { playlistDao.updateSyncStatus(playlistId, any()) }
    }

    @Test
    fun `updatePlaylistTracks default fromServer is false`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When updating tracks without specifying fromServer (default = false)
        store.updatePlaylistTracks(playlistId, listOf("track-1", "track-2"))

        // Then it should mark as PendingUpdate
        coVerify { playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate) }
    }

    @Test
    fun `updatePlaylistTracks does not change status if already PendingCreate`() = runTest {
        // Given a playlist pending creation
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1"),
            syncStatus = PlaylistSyncStatus.PendingCreate
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When updating tracks
        store.updatePlaylistTracks(playlistId, listOf("track-1", "track-2"), fromServer = false)

        // Then it should NOT change sync status (not Synced)
        coVerify { playlistDao.updateTrackIds(playlistId, listOf("track-1", "track-2")) }
        coVerify(exactly = 0) { playlistDao.updateSyncStatus(playlistId, any()) }
    }

    // ================================================================================
    // updatePlaylistName tests
    // ================================================================================

    @Test
    fun `updatePlaylistName with fromServer=false marks synced playlist as PendingUpdate`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "Old Name",
            trackIds = emptyList(),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When updating name with fromServer=false
        store.updatePlaylistName(playlistId, "New Name", fromServer = false)

        // Then it should mark as PendingUpdate
        coVerify { playlistDao.updateName(playlistId, "New Name") }
        coVerify { playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate) }
    }

    @Test
    fun `updatePlaylistName with fromServer=true does NOT mark playlist as PendingUpdate`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "Old Name",
            trackIds = emptyList(),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When updating name with fromServer=true
        store.updatePlaylistName(playlistId, "New Name", fromServer = true)

        // Then it should update name but NOT change sync status
        coVerify { playlistDao.updateName(playlistId, "New Name") }
        coVerify(exactly = 0) { playlistDao.updateSyncStatus(playlistId, any()) }
    }

    // ================================================================================
    // addTrackToPlaylist tests
    // ================================================================================

    @Test
    fun `addTrackToPlaylist marks synced playlist as PendingUpdate`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When adding a track
        store.addTrackToPlaylist(playlistId, "track-2")

        // Then it should mark as PendingUpdate
        coVerify { playlistDao.updateTrackIds(playlistId, listOf("track-1", "track-2")) }
        coVerify { playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate) }
    }

    @Test
    fun `addTrackToPlaylist does not add duplicate track`() = runTest {
        // Given a playlist with the track already present
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1", "track-2"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When adding a track that already exists
        store.addTrackToPlaylist(playlistId, "track-1")

        // Then it should NOT update
        coVerify(exactly = 0) { playlistDao.updateTrackIds(any(), any()) }
        coVerify(exactly = 0) { playlistDao.updateSyncStatus(any(), any()) }
    }

    // ================================================================================
    // removeTrackFromPlaylist tests
    // ================================================================================

    @Test
    fun `removeTrackFromPlaylist marks synced playlist as PendingUpdate`() = runTest {
        // Given a synced playlist
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1", "track-2"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When removing a track
        store.removeTrackFromPlaylist(playlistId, "track-1")

        // Then it should mark as PendingUpdate
        coVerify { playlistDao.updateTrackIds(playlistId, listOf("track-2")) }
        coVerify { playlistDao.updateSyncStatus(playlistId, PlaylistSyncStatus.PendingUpdate) }
    }

    @Test
    fun `removeTrackFromPlaylist does nothing if track not present`() = runTest {
        // Given a playlist without the track
        val playlistId = "playlist-1"
        val existingPlaylist = PlaylistEntity(
            id = playlistId,
            name = "My Playlist",
            trackIds = listOf("track-1", "track-2"),
            syncStatus = PlaylistSyncStatus.Synced
        )
        coEvery { playlistDao.getByIdOnce(playlistId) } returns existingPlaylist

        // When removing a track that doesn't exist
        store.removeTrackFromPlaylist(playlistId, "track-999")

        // Then it should NOT update
        coVerify(exactly = 0) { playlistDao.updateTrackIds(any(), any()) }
        coVerify(exactly = 0) { playlistDao.updateSyncStatus(any(), any()) }
    }
}
