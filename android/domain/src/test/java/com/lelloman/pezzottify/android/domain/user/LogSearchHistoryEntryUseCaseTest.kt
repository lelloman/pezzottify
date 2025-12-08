package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class LogSearchHistoryEntryUseCaseTest {

    private lateinit var userDataStore: UserDataStore
    private lateinit var loggerFactory: LoggerFactory
    private lateinit var logSearchHistoryEntry: LogSearchHistoryEntryUseCase

    @Before
    fun setUp() {
        userDataStore = mockk(relaxed = true)

        val mockLogger = mockk<Logger>(relaxed = true)
        loggerFactory = mockk()
        every { loggerFactory.getLogger(any<String>()) } returns mockLogger
        every { loggerFactory.getValue(any(), any()) } returns mockLogger

        logSearchHistoryEntry = LogSearchHistoryEntryUseCase(userDataStore, loggerFactory)
    }

    @Test
    fun `invoke calls addSearchHistoryEntry with correct parameters for Artist`() = runTest {
        val query = "prince"
        val contentType = SearchHistoryEntry.Type.Artist
        val contentId = "artist-123"

        logSearchHistoryEntry(query, contentType, contentId)
        advanceUntilIdle()

        coVerify {
            userDataStore.addSearchHistoryEntry(query, contentType, contentId)
        }
    }

    @Test
    fun `invoke calls addSearchHistoryEntry with correct parameters for Album`() = runTest {
        val query = "purple rain"
        val contentType = SearchHistoryEntry.Type.Album
        val contentId = "album-456"

        logSearchHistoryEntry(query, contentType, contentId)
        advanceUntilIdle()

        coVerify {
            userDataStore.addSearchHistoryEntry(query, contentType, contentId)
        }
    }

    @Test
    fun `invoke calls addSearchHistoryEntry with correct parameters for Track`() = runTest {
        val query = "when doves cry"
        val contentType = SearchHistoryEntry.Type.Track
        val contentId = "track-789"

        logSearchHistoryEntry(query, contentType, contentId)
        advanceUntilIdle()

        coVerify {
            userDataStore.addSearchHistoryEntry(query, contentType, contentId)
        }
    }
}
