package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.usecase.IsLoggedIn
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogin
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogout
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.ui.screen.player.RepeatModeUi
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.domain.user.GetRecentlyViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.LogViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenState
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.player.PlayerScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.queue.QueueScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map

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
        configStore: ConfigStore,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override fun getInitialHost(): String = configStore.baseUrl.value

        override suspend fun setHost(host: String) {
            configStore.setBaseUrl(host)
        }

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
        authStore: AuthStore,
        configStore: ConfigStore,
        userSettingsStore: UserSettingsStore,
        buildInfo: BuildInfo,
    ): ProfileScreenViewModel.Interactor = object : ProfileScreenViewModel.Interactor {
        override suspend fun logout() {
            performLogout()
        }

        override fun getUserName(): String {
            val authState = authStore.getAuthState().value
            return if (authState is AuthState.LoggedIn) {
                authState.userHandle
            } else {
                ""
            }
        }

        override fun getBaseUrl(): String = configStore.baseUrl.value

        override fun getPlayBehavior(): PlayBehavior = userSettingsStore.playBehavior.value

        override fun getThemeMode(): ThemeMode = userSettingsStore.themeMode.value

        override fun observePlayBehavior() = userSettingsStore.playBehavior

        override fun observeThemeMode() = userSettingsStore.themeMode

        override suspend fun setPlayBehavior(playBehavior: PlayBehavior) {
            userSettingsStore.setPlayBehavior(playBehavior)
        }

        override suspend fun setThemeMode(themeMode: ThemeMode) {
            userSettingsStore.setThemeMode(themeMode)
        }

        override fun getBuildVariant(): String = buildInfo.buildVariant

        override fun getVersionName(): String = buildInfo.versionName

        override fun getGitCommit(): String = buildInfo.gitCommit
    }

    @Provides
    fun provideSearchScreenInteractor(
        performSearch: PerformSearch,
        loggerFactory: LoggerFactory,
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
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        userSettingsStore: UserSettingsStore,
    ): AlbumScreenViewModel.Interactor = object : AlbumScreenViewModel.Interactor {
        override fun playAlbum(albumId: String) {
            when (userSettingsStore.playBehavior.value) {
                PlayBehavior.ReplacePlaylist -> player.loadAlbum(albumId)
                PlayBehavior.AddToPlaylist -> player.addAlbumToPlaylist(albumId)
            }
        }

        override fun playTrack(albumId: String, trackId: String) {
            when (userSettingsStore.playBehavior.value) {
                PlayBehavior.ReplacePlaylist -> player.loadAlbum(albumId, trackId)
                PlayBehavior.AddToPlaylist -> player.addTracksToPlaylist(listOf(trackId))
            }
        }

        override fun logViewedAlbum(albumId: String) {
            logViewedContentUseCase(albumId, ViewedContent.Type.Album)
        }

        override fun getCurrentPlayingTrackId(): Flow<String?> =
            player.playbackPlaylist.combine(player.currentTrackIndex) { playlist, currentTrackIndex ->
                if (playlist != null && currentTrackIndex != null && currentTrackIndex in playlist.tracksIds.indices) {
                    playlist.tracksIds[currentTrackIndex]
                } else {
                    null
                }
            }

        override fun getIsAddToQueueMode(): Flow<Boolean> =
            userSettingsStore.playBehavior.map { it == PlayBehavior.AddToPlaylist }
    }

    @Provides
    fun provideArtistScreenInteractor(
        logViewedContentUseCase: LogViewedContentUseCase,
    ): ArtistScreenViewModel.Interactor = object : ArtistScreenViewModel.Interactor {
        override fun logViewedArtist(artistId: String) {
            logViewedContentUseCase(artistId, ViewedContent.Type.Artist)
        }
    }

    @Provides
    fun provideMainScreenInteractor(
        loggerFactory: LoggerFactory,
        player: PezzottifyPlayer
    ): MainScreenViewModel.Interactor =
        object : MainScreenViewModel.Interactor {

            val logger = loggerFactory.getLogger(MainScreenViewModel.Interactor::class)
            override fun getPlaybackState(): Flow<MainScreenViewModel.Interactor.PlaybackState?> =
                player
                    .playbackPlaylist.combine(player.isPlaying) { playlist, isPlaying -> playlist to isPlaying }
                    .combine(player.currentTrackIndex) { (playlist, isPlaying), currentTrackIndex ->
                        Triple(playlist, isPlaying, currentTrackIndex)
                    }
                    .combine(player.currentTrackPercent) { (playlist, isPlaying, currentTrackIndex), trackPercent ->
                        logger.debug("Combining new playlist + isPlaying + currentTrackIndex + trackPercent $playlist - $isPlaying - $currentTrackIndex - $trackPercent")
                        if (playlist != null) {
                            val index = currentTrackIndex ?: 0
                            val nextTrackId = if (index < playlist.tracksIds.lastIndex) {
                                playlist.tracksIds[index + 1]
                            } else null
                            val previousTrackId = if (index > 0) {
                                playlist.tracksIds[index - 1]
                            } else null
                            MainScreenViewModel.Interactor.PlaybackState.Loaded(
                                isPlaying = isPlaying,
                                trackId = playlist.tracksIds[index],
                                trackPercent = trackPercent ?: 0f,
                                nextTrackId = nextTrackId,
                                previousTrackId = previousTrackId,
                            )
                        } else {
                            null
                        }
                    }

            override fun clickOnPlayPause() = player.togglePlayPause()

            override fun clickOnSkipToNext() = player.skipToNextTrack()

            override fun clickOnSkipToPrevious() = player.skipToPreviousTrack()
        }

    @Provides
    fun provideHomeScreenInteractor(getRecentlyViewedContent: GetRecentlyViewedContentUseCase) =
        object : HomeScreenViewModel.Interactor {
            override suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<HomeScreenState.RecentlyViewedContent>> =
                getRecentlyViewedContent(maxCount).map {
                    it.map { item ->
                        val type = when (item.type) {
                            ViewedContent.Type.Album -> ViewedContentType.Album
                            ViewedContent.Type.Artist -> ViewedContentType.Artist
                            ViewedContent.Type.Track -> ViewedContentType.Track
                            ViewedContent.Type.UserPlaylist -> ViewedContentType.Playlist
                        }
                        HomeScreenState.RecentlyViewedContent(item.contentId, type)
                    }
                }
        }

    @Provides
    fun providePlayerScreenInteractor(
        player: PezzottifyPlayer
    ): PlayerScreenViewModel.Interactor =
        object : PlayerScreenViewModel.Interactor {
            override fun getPlaybackState(): Flow<PlayerScreenViewModel.Interactor.PlaybackState?> =
                player.playbackPlaylist
                    .combine(player.isPlaying) { playlist, isPlaying -> playlist to isPlaying }
                    .combine(player.currentTrackIndex) { (playlist, isPlaying), currentTrackIndex ->
                        Triple(playlist, isPlaying, currentTrackIndex)
                    }
                    .combine(player.currentTrackPercent) { (playlist, isPlaying, currentTrackIndex), trackPercent ->
                        data class TempState(
                            val playlist: com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist?,
                            val isPlaying: Boolean,
                            val currentTrackIndex: Int?,
                            val trackPercent: Float?,
                        )
                        TempState(playlist, isPlaying, currentTrackIndex, trackPercent)
                    }
                    .combine(player.currentTrackProgressSec) { tempState, progressSec ->
                        data class TempState2(
                            val playlist: com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist?,
                            val isPlaying: Boolean,
                            val currentTrackIndex: Int?,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                        )
                        TempState2(tempState.playlist, tempState.isPlaying, tempState.currentTrackIndex, tempState.trackPercent, progressSec)
                    }
                    .combine(player.volumeState) { tempState, volumeState ->
                        data class TempState3(
                            val playlist: com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist?,
                            val isPlaying: Boolean,
                            val currentTrackIndex: Int?,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                            val volume: Float,
                            val isMuted: Boolean,
                        )
                        TempState3(tempState.playlist, tempState.isPlaying, tempState.currentTrackIndex, tempState.trackPercent, tempState.progressSec, volumeState.volume, volumeState.isMuted)
                    }
                    .combine(player.shuffleEnabled) { tempState, shuffleEnabled ->
                        data class TempState4(
                            val playlist: com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist?,
                            val isPlaying: Boolean,
                            val currentTrackIndex: Int?,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                            val volume: Float,
                            val isMuted: Boolean,
                            val shuffleEnabled: Boolean,
                        )
                        TempState4(tempState.playlist, tempState.isPlaying, tempState.currentTrackIndex, tempState.trackPercent, tempState.progressSec, tempState.volume, tempState.isMuted, shuffleEnabled)
                    }
                    .combine(player.repeatMode) { tempState, repeatMode ->
                        if (tempState.playlist != null) {
                            val index = tempState.currentTrackIndex ?: 0
                            val hasNext = index < tempState.playlist.tracksIds.lastIndex
                            val hasPrevious = index > 0
                            val repeatModeUi = when (repeatMode) {
                                RepeatMode.OFF -> RepeatModeUi.OFF
                                RepeatMode.ALL -> RepeatModeUi.ALL
                                RepeatMode.ONE -> RepeatModeUi.ONE
                            }
                            PlayerScreenViewModel.Interactor.PlaybackState.Loaded(
                                isPlaying = tempState.isPlaying,
                                trackId = tempState.playlist.tracksIds[index],
                                trackPercent = tempState.trackPercent ?: 0f,
                                trackProgressSec = tempState.progressSec ?: 0,
                                hasNextTrack = hasNext,
                                hasPreviousTrack = hasPrevious,
                                volume = tempState.volume,
                                isMuted = tempState.isMuted,
                                shuffleEnabled = tempState.shuffleEnabled,
                                repeatMode = repeatModeUi,
                            )
                        } else {
                            null
                        }
                    }

            override fun togglePlayPause() = player.togglePlayPause()

            override fun skipToNext() = player.skipToNextTrack()

            override fun skipToPrevious() = player.skipToPreviousTrack()

            override fun seekToPercent(percent: Float) = player.seekToPercentage(percent)

            override fun setVolume(volume: Float) = player.setVolume(volume)

            override fun toggleMute() {
                val currentState = player.volumeState.value
                player.setMuted(!currentState.isMuted)
            }

            override fun toggleShuffle() = player.toggleShuffle()

            override fun cycleRepeatMode() = player.cycleRepeatMode()
        }

    @Provides
    fun provideQueueScreenInteractor(
        player: PezzottifyPlayer
    ): QueueScreenViewModel.Interactor =
        object : QueueScreenViewModel.Interactor {
            override fun getPlaybackPlaylist() = player.playbackPlaylist

            override fun getCurrentTrackIndex() = player.currentTrackIndex

            override fun playTrackAtIndex(index: Int) = player.loadTrackIndex(index)

            override fun moveTrack(fromIndex: Int, toIndex: Int) = player.moveTrack(fromIndex, toIndex)

            override fun removeTrack(trackId: String) = player.removeTrackFromPlaylist(trackId)
        }
}