package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.auth.usecase.IsLoggedIn
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogin
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogout
import com.lelloman.pezzottify.android.domain.player.Player
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine

@InstallIn(ViewModelComponent::class)
@Module
class InteractorsModule {

    @Provides
    fun provideSplashInteractor(isLoggedIn: IsLoggedIn): SplashViewModel.Interactor =
        object : SplashViewModel.Interactor {
            override suspend fun isLoggedIn() = isLoggedIn()
        }

    @Provides
    fun provideLoginInteractor(
        performLogin: PerformLogin,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override suspend fun login(
            email: String,
            password: String
        ): LoginViewModel.Interactor.LoginResult =
            when (performLogin(email, password)) {
                PerformLogin.LoginResult.Success -> LoginViewModel.Interactor.LoginResult.Success
                PerformLogin.LoginResult.WrongCredentials -> LoginViewModel.Interactor.LoginResult.Failure.InvalidCredentials
                PerformLogin.LoginResult.Error -> LoginViewModel.Interactor.LoginResult.Failure.Unknown
            }
    }

    @Provides
    fun provideProfileScreenInteractor(
        performLogout: PerformLogout,
    ): ProfileScreenViewModel.Interactor = object : ProfileScreenViewModel.Interactor {
        override suspend fun logout() {
            performLogout()
        }
    }

    @Provides
    fun provideSearchScreenInteractor(
        performSearch: PerformSearch,
        loggerFactory: LoggerFactory
    ): SearchScreenViewModel.Interactor =
        object : SearchScreenViewModel.Interactor {
            private val logger = loggerFactory.getLogger("SearchScreenViewModel.Interactor")

            override suspend fun search(query: String): Result<List<Pair<String, SearchScreenViewModel.SearchedItemType>>> {
                logger.debug("search($query)")
                val performSearchResult = performSearch(query)
                if (performSearchResult.isFailure) {
                    logger.debug("search($query) returning failure")
                    return Result.failure(
                        performSearchResult.exceptionOrNull() ?: Throwable("PerformSearch failed")
                    )
                }
                val searchResult = performSearchResult.getOrNull() ?: emptyList()
                val mappedResult = searchResult.map {
                    it.first to when (it.second) {
                        com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType.Album -> SearchScreenViewModel.SearchedItemType.Album
                        com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType.Track -> SearchScreenViewModel.SearchedItemType.Track
                        com.lelloman.pezzottify.android.domain.remoteapi.response.SearchedItemType.Artist -> SearchScreenViewModel.SearchedItemType.Artist

                    }
                }
                logger.debug("search($query) returning $mappedResult")
                return Result.success(mappedResult)
            }
        }

    @Provides
    fun provideAlbumScreenInteractor(
        player: Player
    ): AlbumScreenViewModel.Interactor = object : AlbumScreenViewModel.Interactor {
        override fun playAlbum(albumId: String) {
            player.loadAlbum(albumId)
        }
    }

    @Provides
    fun provideMainScreenInteractor(
        loggerFactory: LoggerFactory,
        player: Player
    ): MainScreenViewModel.Interactor =
        object : MainScreenViewModel.Interactor {

            val logger = loggerFactory.getLogger(MainScreenViewModel.Interactor::class)
            override fun getPlaybackState(): Flow<MainScreenViewModel.PlaybackState?> = player
                .playbackPlaylist.combine(player.isPlaying) { playlist, isPlaying ->
                    logger.debug("Combining new playlist + isPlaying $playlist - $isPlaying")
                    if (playlist != null) {
                        MainScreenViewModel.PlaybackState(
                            isPlaying = isPlaying,
                            trackId = playlist.tracksIds[playlist.currentTrackIndex ?: 0],
                            trackPercent = 0f,
                        )
                    } else {
                        null
                    }
                }

            override fun clickOnPlayPause() = player.togglePlayPause()

            override fun clickOnSkipToNext() = player.skipToNextTrack()

            override fun clickOnSkipToPrevious() = player.skipToPreviousTrack()
        }
}