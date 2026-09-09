package com.lelloman.pezzottify.android.ui

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.statics.FeaturedAlbum
import com.lelloman.pezzottify.android.domain.statics.FeaturedAlbums
import com.lelloman.pezzottify.android.domain.statics.PopularAlbum
import com.lelloman.pezzottify.android.domain.statics.PopularContent
import com.lelloman.pezzottify.android.domain.statics.usecase.GetFeaturedAlbums
import com.lelloman.pezzottify.android.domain.statics.usecase.GetPopularContent
import com.lelloman.pezzottify.android.domain.user.GetRecentlyViewedContentUseCase
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import io.mockk.coEvery
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.async
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class HomeScreenInteractorTest {

    @Test
    fun `remote home sections wait for authentication restoration`() = runTest {
        val authState = MutableStateFlow<AuthState>(AuthState.Loading)
        val authStore = mockk<AuthStore>(relaxed = true) {
            every { getAuthState() } returns authState
        }
        val featuredAlbums = mockk<GetFeaturedAlbums>()
        coEvery { featuredAlbums.invoke(any()) } returns Result.success(
            FeaturedAlbums(
                heroIndex = 0,
                albums = listOf(FeaturedAlbum("featured-1", "Featured", listOf("Artist"))),
            )
        )
        val popularContent = mockk<GetPopularContent>()
        coEvery { popularContent.invoke(any(), any()) } returns Result.success(
            PopularContent(
                albums = listOf(PopularAlbum("popular-1", "Popular", listOf("Artist"))),
                artists = emptyList(),
            )
        )
        val configStore = mockk<ConfigStore> {
            every { baseUrl } returns MutableStateFlow("https://example.test")
        }
        val interactor = InteractorsModule().provideHomeScreenInteractor(
            getRecentlyViewedContent = mockk<GetRecentlyViewedContentUseCase>(),
            getFeaturedAlbums = featuredAlbums,
            getPopularContentUseCase = popularContent,
            authStore = authStore,
            webSocketManager = mockk<WebSocketManager>(),
            configStore = configStore,
        )

        val featuredResult = async { interactor.getFeaturedContent() }
        val popularResult = async { interactor.getPopularContent() }
        runCurrent()

        assertThat(featuredResult.isCompleted).isFalse()
        assertThat(popularResult.isCompleted).isFalse()

        authState.value = AuthState.LoggedIn(
            userHandle = "user",
            authToken = "token",
            remoteUrl = "https://example.test",
        )

        assertThat(featuredResult.await()?.albums?.single()?.id).isEqualTo("featured-1")
        assertThat(popularResult.await()?.albums?.single()?.id).isEqualTo("popular-1")
    }
}
