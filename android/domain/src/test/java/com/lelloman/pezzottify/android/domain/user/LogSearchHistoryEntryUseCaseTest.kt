package com.lelloman.pezzottify.android.domain.user

import com.lelloman.pezzottify.android.logger.Logger
import io.mockk.coVerify
import io.mockk.mockk
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class LogSearchHistoryEntryUseCaseTest {

    private lateinit var userDataStore: UserDataStore
    private lateinit var logger: Logger
    private val testDispatcher = StandardTestDispatcher()
    private val testScope = TestScope(testDispatcher)

    private lateinit var logSearchHistoryEntry: LogSearchHistoryEntryUseCase

    @Before
    fun setUp() {
        userDataStore = mockk(relaxed = true)
        logger = mockk(relaxed = true)

        logSearchHistoryEntry = LogSearchHistoryEntryUseCase(
            userDataStore = userDataStore,
            scope = testScope,
            dispatcher = testDispatcher,
            logger = logger,
        )
    }

    @Test
    fun `invoke calls addSearchHistoryEntry with correct parameters for Artist`() = testScope.runTest {
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
    fun `invoke calls addSearchHistoryEntry with correct parameters for Album`() = testScope.runTest {
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
    fun `invoke calls addSearchHistoryEntry with correct parameters for Track`() = testScope.runTest {
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
