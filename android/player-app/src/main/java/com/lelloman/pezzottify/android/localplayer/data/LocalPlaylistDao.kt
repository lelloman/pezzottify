package com.lelloman.pezzottify.android.localplayer.data

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Transaction
import kotlinx.coroutines.flow.Flow

data class LocalPlaylistWithCount(
    val id: String,
    val name: String,
    val trackCount: Int,
    val createdAt: Long
)

@Dao
interface LocalPlaylistDao {

    @Query("""
        SELECT p.id, p.name, p.createdAt, COUNT(t.playlistId) as trackCount
        FROM local_playlist p
        LEFT JOIN local_playlist_track t ON p.id = t.playlistId
        GROUP BY p.id
        ORDER BY p.createdAt DESC
    """)
    fun getAllPlaylistsWithCount(): Flow<List<LocalPlaylistWithCount>>

    @Query("SELECT * FROM local_playlist WHERE id = :playlistId")
    suspend fun getPlaylist(playlistId: String): LocalPlaylistEntity?

    @Query("SELECT * FROM local_playlist_track WHERE playlistId = :playlistId ORDER BY position")
    suspend fun getPlaylistTracks(playlistId: String): List<LocalPlaylistTrackEntity>

    @Insert
    suspend fun insertPlaylist(playlist: LocalPlaylistEntity)

    @Insert
    suspend fun insertTracks(tracks: List<LocalPlaylistTrackEntity>)

    @Transaction
    suspend fun insertPlaylistWithTracks(
        playlist: LocalPlaylistEntity,
        tracks: List<LocalPlaylistTrackEntity>
    ) {
        insertPlaylist(playlist)
        insertTracks(tracks)
    }

    @Query("DELETE FROM local_playlist WHERE id = :playlistId")
    suspend fun deletePlaylist(playlistId: String)

    @Query("DELETE FROM local_playlist_track WHERE playlistId = :playlistId")
    suspend fun deletePlaylistTracks(playlistId: String)
}
