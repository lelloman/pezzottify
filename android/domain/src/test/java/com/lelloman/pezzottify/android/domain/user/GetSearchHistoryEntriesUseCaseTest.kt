package com.lelloman.pezzottify.android.domain.user

import com.google.common.truth.Truth.assertThat
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class GetSearchHistoryEntriesUseCaseTest {

    private lateinit var userDataStore: UserDataStore
    private lateinit var getSearchHistoryEntries: GetSearchHistoryEntriesUseCase

    @Before
    fun setUp() {
        userDataStore = mockk()
        getSearchHistoryEntries = GetSearchHistoryEntriesUseCase(userDataStore)
    }

    @Test
    fun `invoke returns flow from userDataStore`() = runTest {
        val entries = listOf(
            SearchHistoryEntry(
                query = "prince",
                contentType = SearchHistoryEntry.Type.Artist,
                contentId = "artist-123",
                created = 1000L,
            ),
            SearchHistoryEntry(
                query = "purple rain",
                contentType = SearchHistoryEntry.Type.Album,
                contentId = "album-456",
                created = 2000L,
            ),
        )
        every { userDataStore.getSearchHistoryEntries(10) } returns flowOf(entries)

        val result = getSearchHistoryEntries(10).first()

        assertThat(result).isEqualTo(entries)
        verify { userDataStore.getSearchHistoryEntries(10) }
    }

    @Test
    fun `invoke passes limit to userDataStore`() = runTest {
        every { userDataStore.getSearchHistoryEntries(5) } returns flowOf(emptyList())

        getSearchHistoryEntries(5).first()

        verify { userDataStore.getSearchHistoryEntries(5) }
    }

    @Test
    fun `invoke returns empty list when no entries exist`() = runTest {
        every { userDataStore.getSearchHistoryEntries(10) } returns flowOf(emptyList())

        val result = getSearchHistoryEntries(10).first()

        assertThat(result).isEmpty()
    }
}
