package com.lelloman.pezzottify.android.localdata.internal.usercontent

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import com.lelloman.pezzottify.android.localdata.internal.usercontent.model.LikedContentEntity
import io.mockk.coVerify
import io.mockk.mockk
import io.mockk.slot
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class UserContentStoreImplTest {

    private lateinit var likedContentDao: LikedContentDao
    private lateinit var store: UserContentStoreImpl

    @Before
    fun setup() {
        likedContentDao = mockk(relaxed = true)
        store = UserContentStoreImpl(likedContentDao)
    }

    // ================================================================================
    // setLiked tests
    // ================================================================================

    @Test
    fun `setLiked with Synced status should persist Synced status`() = runTest {
        // Given
        val entitySlot = slot<LikedContentEntity>()

        // When
        store.setLiked(
            contentId = "track-1",
            type = LikedContent.ContentType.Track,
            liked = true,
            modifiedAt = 1000L,
            syncStatus = SyncStatus.Synced,
        )

        // Then the entity passed to the DAO should have Synced status
        coVerify { likedContentDao.upsert(capture(entitySlot)) }
        assertThat(entitySlot.captured.syncStatus).isEqualTo(SyncStatus.Synced)
    }

    @Test
    fun `setLiked with PendingSync status should persist PendingSync status`() = runTest {
        // Given
        val entitySlot = slot<LikedContentEntity>()

        // When
        store.setLiked(
            contentId = "album-1",
            type = LikedContent.ContentType.Album,
            liked = true,
            modifiedAt = 2000L,
            syncStatus = SyncStatus.PendingSync,
        )

        // Then the entity passed to the DAO should have PendingSync status
        coVerify { likedContentDao.upsert(capture(entitySlot)) }
        assertThat(entitySlot.captured.syncStatus).isEqualTo(SyncStatus.PendingSync)
    }

    @Test
    fun `setLiked should pass through all parameters correctly`() = runTest {
        // Given
        val entitySlot = slot<LikedContentEntity>()

        // When
        store.setLiked(
            contentId = "artist-42",
            type = LikedContent.ContentType.Artist,
            liked = false,
            modifiedAt = 9999L,
            syncStatus = SyncStatus.Synced,
        )

        // Then all fields should be forwarded correctly
        coVerify { likedContentDao.upsert(capture(entitySlot)) }
        val captured = entitySlot.captured
        assertThat(captured.contentId).isEqualTo("artist-42")
        assertThat(captured.contentType).isEqualTo(LikedContent.ContentType.Artist)
        assertThat(captured.isLiked).isFalse()
        assertThat(captured.modifiedAt).isEqualTo(9999L)
        assertThat(captured.syncStatus).isEqualTo(SyncStatus.Synced)
    }
}
