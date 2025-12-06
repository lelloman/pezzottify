package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.usecase.IsLoggedIn
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogin
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogout
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState as DomainConnectionState
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.ui.component.ConnectionState as UiConnectionState
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.sync.Permission as DomainPermission
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.ui.model.Permission as UiPermission
import com.lelloman.pezzottify.android.ui.screen.player.RepeatModeUi
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily as DomainAppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette as DomainColorPalette
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior as DomainPlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode as DomainThemeMode
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.storage.StorageInfo as DomainStorageInfo
import com.lelloman.pezzottify.android.domain.storage.StoragePressureLevel as DomainStoragePressureLevel
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent as DomainLikedContent
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist as DomainPlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext as DomainPlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.settings.usecase.UpdateDirectDownloadsSetting
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily as UiAppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette as UiColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode as UiThemeMode
import com.lelloman.pezzottify.android.ui.model.PlayBehavior as UiPlayBehavior
import com.lelloman.pezzottify.android.ui.model.StorageInfo as UiStorageInfo
import com.lelloman.pezzottify.android.ui.model.StoragePressureLevel as UiStoragePressureLevel
import com.lelloman.pezzottify.android.ui.model.LikedContent as UiLikedContent
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylist as UiPlaybackPlaylist
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylistContext as UiPlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.domain.user.GetRecentlyViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.GetSearchHistoryEntriesUseCase
import com.lelloman.pezzottify.android.domain.user.LogSearchHistoryEntryUseCase
import com.lelloman.pezzottify.android.domain.user.LogViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.domain.usercontent.GetLikedStateUseCase
import com.lelloman.pezzottify.android.domain.usercontent.ToggleLikeUseCase
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenState
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import com.lelloman.pezzottify.android.ui.screen.main.library.LibraryScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings.StyleSettingsViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.settings.SettingsScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.player.PlayerScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.queue.QueueScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn

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
        authStore: AuthStore,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override fun getInitialHost(): String =
            authStore.getLastUsedBaseUrl() ?: configStore.baseUrl.value

        override fun getInitialEmail(): String =
            authStore.getLastUsedHandle() ?: ""

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
        buildInfo: BuildInfo,
        webSocketManager: WebSocketManager,
        permissionsStore: PermissionsStore,
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

        override fun getBuildVariant(): String = buildInfo.buildVariant

        override fun getVersionName(): String = buildInfo.versionName

        override fun getGitCommit(): String = buildInfo.gitCommit

        override fun observeServerVersion() = webSocketManager.connectionState.map { state ->
            when (state) {
                is DomainConnectionState.Connected -> state.serverVersion
                else -> "disconnected"
            }
        }

        override fun observePermissions(): Flow<Set<UiPermission>> =
            permissionsStore.permissions.map { permissions ->
                permissions.mapNotNull { it.toUi() }.toSet()
            }
    }

    @Provides
    fun provideSettingsScreenInteractor(
        userSettingsStore: UserSettingsStore,
        storageMonitor: com.lelloman.pezzottify.android.domain.storage.StorageMonitor,
        permissionsStore: PermissionsStore,
        updateDirectDownloadsSetting: UpdateDirectDownloadsSetting,
    ): SettingsScreenViewModel.Interactor = object : SettingsScreenViewModel.Interactor {
        override fun getPlayBehavior(): UiPlayBehavior =
            userSettingsStore.playBehavior.value.toUi()

        override fun getThemeMode(): UiThemeMode = userSettingsStore.themeMode.value.toUi()

        override fun getColorPalette(): UiColorPalette = userSettingsStore.colorPalette.value.toUi()

        override fun getFontFamily(): UiAppFontFamily = userSettingsStore.fontFamily.value.toUi()

        override fun isCacheEnabled(): Boolean = userSettingsStore.isInMemoryCacheEnabled.value

        override fun getStorageInfo(): UiStorageInfo = storageMonitor.storageInfo.value.toUi()

        override fun isDirectDownloadsEnabled(): Boolean = userSettingsStore.directDownloadsEnabled.value

        override fun hasIssueContentDownloadPermission(): Boolean =
            permissionsStore.permissions.value.contains(DomainPermission.IssueContentDownload)

        override fun observePlayBehavior(): Flow<UiPlayBehavior> = userSettingsStore.playBehavior.map { it.toUi() }

        override fun observeThemeMode(): Flow<UiThemeMode> = userSettingsStore.themeMode.map { it.toUi()}

        override fun observeColorPalette(): Flow<UiColorPalette>  = userSettingsStore.colorPalette.map { it.toUi() }

        override fun observeFontFamily(): Flow<UiAppFontFamily> = userSettingsStore.fontFamily.map { it.toUi() }

        override fun observeCacheEnabled() = userSettingsStore.isInMemoryCacheEnabled

        override fun observeStorageInfo(): Flow<UiStorageInfo> = storageMonitor.storageInfo.map { it.toUi() }

        override fun observeDirectDownloadsEnabled(): Flow<Boolean> = userSettingsStore.directDownloadsEnabled

        override fun observeHasIssueContentDownloadPermission(): Flow<Boolean> =
            permissionsStore.permissions.map { it.contains(DomainPermission.IssueContentDownload) }

        override suspend fun setPlayBehavior(playBehavior: UiPlayBehavior) {
            userSettingsStore.setPlayBehavior(playBehavior.toDomain())
        }

        override suspend fun setThemeMode(themeMode: UiThemeMode) {
            userSettingsStore.setThemeMode(themeMode.toDomain())
        }

        override suspend fun setColorPalette(colorPalette: UiColorPalette) {
            userSettingsStore.setColorPalette(colorPalette.toDomain())
        }

        override suspend fun setFontFamily(fontFamily: UiAppFontFamily) {
            userSettingsStore.setFontFamily(fontFamily.toDomain())
        }

        override suspend fun setCacheEnabled(enabled: Boolean) {
            userSettingsStore.setInMemoryCacheEnabled(enabled)
        }

        override suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean =
            when (updateDirectDownloadsSetting(enabled)) {
                UpdateDirectDownloadsSetting.Result.Success -> true
                UpdateDirectDownloadsSetting.Result.Error -> false
            }
    }

    @Provides
    fun provideStyleSettingsInteractor(
        userSettingsStore: UserSettingsStore,
    ): StyleSettingsViewModel.Interactor = object : StyleSettingsViewModel.Interactor {
        override fun getThemeMode(): UiThemeMode = userSettingsStore.themeMode.value.toUi()

        override fun getColorPalette(): UiColorPalette = userSettingsStore.colorPalette.value.toUi()

        override fun getFontFamily(): UiAppFontFamily = userSettingsStore.fontFamily.value.toUi()

        override fun observeThemeMode(): Flow<UiThemeMode> = userSettingsStore.themeMode.map { it.toUi() }

        override fun observeColorPalette(): Flow<UiColorPalette> = userSettingsStore.colorPalette.map { it.toUi() }

        override fun observeFontFamily(): Flow<UiAppFontFamily> = userSettingsStore.fontFamily.map { it.toUi() }

        override suspend fun setThemeMode(themeMode: UiThemeMode) {
            userSettingsStore.setThemeMode(themeMode.toDomain())
        }

        override suspend fun setColorPalette(colorPalette: UiColorPalette) {
            userSettingsStore.setColorPalette(colorPalette.toDomain())
        }

        override suspend fun setFontFamily(fontFamily: UiAppFontFamily) {
            userSettingsStore.setFontFamily(fontFamily.toDomain())
        }
    }

    @Provides
    fun provideSearchScreenInteractor(
        performSearch: PerformSearch,
        loggerFactory: LoggerFactory,
        getRecentlyViewedContent: GetRecentlyViewedContentUseCase,
        getSearchHistoryEntries: GetSearchHistoryEntriesUseCase,
        logSearchHistoryEntry: LogSearchHistoryEntryUseCase,
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

            override suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<SearchScreenViewModel.RecentlyViewedContent>> =
                getRecentlyViewedContent(maxCount).map {
                    it.map { item ->
                        val type = when (item.type) {
                            ViewedContent.Type.Album -> ViewedContentType.Album
                            ViewedContent.Type.Artist -> ViewedContentType.Artist
                            ViewedContent.Type.Track -> ViewedContentType.Track
                            ViewedContent.Type.UserPlaylist -> ViewedContentType.Playlist
                        }
                        SearchScreenViewModel.RecentlyViewedContent(item.contentId, type)
                    }
                }

            override fun getSearchHistoryEntries(maxCount: Int): Flow<List<SearchScreenViewModel.SearchHistoryEntry>> =
                getSearchHistoryEntries(maxCount).map {
                    it.map { item ->
                        val type = when (item.contentType) {
                            SearchHistoryEntry.Type.Album -> ViewedContentType.Album
                            SearchHistoryEntry.Type.Artist -> ViewedContentType.Artist
                            SearchHistoryEntry.Type.Track -> ViewedContentType.Track
                        }
                        SearchScreenViewModel.SearchHistoryEntry(item.query, item.contentId, type)
                    }
                }

            override fun logSearchHistoryEntry(
                query: String,
                contentType: SearchScreenViewModel.SearchHistoryEntryType,
                contentId: String
            ) {
                val domainType = when (contentType) {
                    SearchScreenViewModel.SearchHistoryEntryType.Album -> SearchHistoryEntry.Type.Album
                    SearchScreenViewModel.SearchHistoryEntryType.Artist -> SearchHistoryEntry.Type.Artist
                    SearchScreenViewModel.SearchHistoryEntryType.Track -> SearchHistoryEntry.Type.Track
                }
                logSearchHistoryEntry(query, domainType, contentId)
            }
        }

    @Provides
    fun provideAlbumScreenInteractor(
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        userSettingsStore: UserSettingsStore,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
    ): AlbumScreenViewModel.Interactor = object : AlbumScreenViewModel.Interactor {
        override fun playAlbum(albumId: String) {
            when (userSettingsStore.playBehavior.value) {
                DomainPlayBehavior.ReplacePlaylist -> player.loadAlbum(albumId)
                DomainPlayBehavior.AddToPlaylist -> player.addAlbumToPlaylist(albumId)
            }
        }

        override fun playTrack(albumId: String, trackId: String) {
            when (userSettingsStore.playBehavior.value) {
                DomainPlayBehavior.ReplacePlaylist -> player.loadAlbum(albumId, trackId)
                DomainPlayBehavior.AddToPlaylist -> player.addTracksToPlaylist(listOf(trackId))
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
            userSettingsStore.playBehavior.map { it == DomainPlayBehavior.AddToPlaylist }

        override fun isLiked(contentId: String): Flow<Boolean> =
            getLikedStateUseCase(contentId)

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Album, currentlyLiked)
        }
    }

    @Provides
    fun provideArtistScreenInteractor(
        logViewedContentUseCase: LogViewedContentUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
    ): ArtistScreenViewModel.Interactor = object : ArtistScreenViewModel.Interactor {
        override fun logViewedArtist(artistId: String) {
            logViewedContentUseCase(artistId, ViewedContent.Type.Artist)
        }

        override fun isLiked(contentId: String): Flow<Boolean> =
            getLikedStateUseCase(contentId)

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Artist, currentlyLiked)
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
    fun provideHomeScreenInteractor(
        getRecentlyViewedContent: GetRecentlyViewedContentUseCase,
        authStore: AuthStore,
        webSocketManager: WebSocketManager,
    ) = object : HomeScreenViewModel.Interactor {

        override fun connectionState(scope: CoroutineScope): StateFlow<UiConnectionState> =
            webSocketManager.connectionState.map { it.toUi() }
                .stateIn(scope, SharingStarted.Eagerly, webSocketManager.connectionState.value.toUi())

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

        override fun getUserName(): String {
            val authState = authStore.getAuthState().value
            return if (authState is AuthState.LoggedIn) {
                authState.userHandle
            } else {
                ""
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
                        TempState2(
                            tempState.playlist,
                            tempState.isPlaying,
                            tempState.currentTrackIndex,
                            tempState.trackPercent,
                            progressSec
                        )
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
                        TempState3(
                            tempState.playlist,
                            tempState.isPlaying,
                            tempState.currentTrackIndex,
                            tempState.trackPercent,
                            tempState.progressSec,
                            volumeState.volume,
                            volumeState.isMuted
                        )
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
                        TempState4(
                            tempState.playlist,
                            tempState.isPlaying,
                            tempState.currentTrackIndex,
                            tempState.trackPercent,
                            tempState.progressSec,
                            tempState.volume,
                            tempState.isMuted,
                            shuffleEnabled
                        )
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
            override fun getPlaybackPlaylist(): Flow<UiPlaybackPlaylist?> =
                player.playbackPlaylist.map { it?.toUi() }

            override fun getCurrentTrackIndex() = player.currentTrackIndex

            override fun playTrackAtIndex(index: Int) = player.loadTrackIndex(index)

            override fun moveTrack(fromIndex: Int, toIndex: Int) =
                player.moveTrack(fromIndex, toIndex)

            override fun removeTrack(trackId: String) = player.removeTrackFromPlaylist(trackId)
        }

    @Provides
    fun provideLibraryScreenInteractor(
        userContentStore: UserContentStore,
    ): LibraryScreenViewModel.Interactor =
        object : LibraryScreenViewModel.Interactor {
            override fun getLikedContent(): Flow<List<UiLikedContent>> =
                userContentStore.getLikedContent().map { it.toUi() }
        }

}

private fun DomainThemeMode.toUi(): UiThemeMode = when (this) {
    DomainThemeMode.System -> UiThemeMode.System
    DomainThemeMode.Light -> UiThemeMode.Light
    DomainThemeMode.Dark -> UiThemeMode.Dark
    DomainThemeMode.Amoled -> UiThemeMode.Amoled
}

private fun DomainColorPalette.toUi(): UiColorPalette = when (this) {
    DomainColorPalette.Classic -> UiColorPalette.Classic
    DomainColorPalette.OceanBlue -> UiColorPalette.OceanBlue
    DomainColorPalette.SunsetCoral -> UiColorPalette.SunsetCoral
    DomainColorPalette.PurpleHaze -> UiColorPalette.PurpleHaze
    DomainColorPalette.RoseGold -> UiColorPalette.RoseGold
    DomainColorPalette.Midnight -> UiColorPalette.Midnight
    DomainColorPalette.Forest -> UiColorPalette.Forest
}

private fun DomainAppFontFamily.toUi(): UiAppFontFamily = when (this) {
    DomainAppFontFamily.System -> UiAppFontFamily.System
    DomainAppFontFamily.SansSerif -> UiAppFontFamily.SansSerif
    DomainAppFontFamily.Serif -> UiAppFontFamily.Serif
    DomainAppFontFamily.Monospace -> UiAppFontFamily.Monospace
}

private fun DomainPlayBehavior.toUi(): UiPlayBehavior = when (this) {
    DomainPlayBehavior.ReplacePlaylist -> UiPlayBehavior.ReplacePlaylist
    DomainPlayBehavior.AddToPlaylist -> UiPlayBehavior.AddToPlaylist
}

private fun UiPlayBehavior.toDomain(): DomainPlayBehavior = when (this) {
    UiPlayBehavior.ReplacePlaylist -> DomainPlayBehavior.ReplacePlaylist
    UiPlayBehavior.AddToPlaylist -> DomainPlayBehavior.AddToPlaylist
}

private fun UiThemeMode.toDomain(): DomainThemeMode = when (this) {
    UiThemeMode.System -> DomainThemeMode.System
    UiThemeMode.Light -> DomainThemeMode.Light
    UiThemeMode.Dark -> DomainThemeMode.Dark
    UiThemeMode.Amoled -> DomainThemeMode.Amoled
}

private fun UiColorPalette.toDomain(): DomainColorPalette = when (this) {
    UiColorPalette.Classic -> DomainColorPalette.Classic
    UiColorPalette.OceanBlue -> DomainColorPalette.OceanBlue
    UiColorPalette.SunsetCoral -> DomainColorPalette.SunsetCoral
    UiColorPalette.PurpleHaze -> DomainColorPalette.PurpleHaze
    UiColorPalette.RoseGold -> DomainColorPalette.RoseGold
    UiColorPalette.Midnight -> DomainColorPalette.Midnight
    UiColorPalette.Forest -> DomainColorPalette.Forest
}

private fun UiAppFontFamily.toDomain(): DomainAppFontFamily = when (this) {
    UiAppFontFamily.System -> DomainAppFontFamily.System
    UiAppFontFamily.SansSerif -> DomainAppFontFamily.SansSerif
    UiAppFontFamily.Serif -> DomainAppFontFamily.Serif
    UiAppFontFamily.Monospace -> DomainAppFontFamily.Monospace
}

private fun DomainStoragePressureLevel.toUi(): UiStoragePressureLevel = when (this) {
    DomainStoragePressureLevel.LOW -> UiStoragePressureLevel.LOW
    DomainStoragePressureLevel.MEDIUM -> UiStoragePressureLevel.MEDIUM
    DomainStoragePressureLevel.HIGH -> UiStoragePressureLevel.HIGH
    DomainStoragePressureLevel.CRITICAL -> UiStoragePressureLevel.CRITICAL
}

private fun DomainStorageInfo.toUi(): UiStorageInfo = UiStorageInfo(
    totalBytes = totalBytes,
    availableBytes = availableBytes,
    usedBytes = usedBytes,
    pressureLevel = pressureLevel.toUi()
)

private fun DomainLikedContent.toUi(): UiLikedContent = UiLikedContent(
    contentId = contentId,
    contentType = contentType.toUi(),
    isLiked = isLiked
)

private fun DomainLikedContent.ContentType.toUi(): UiLikedContent.ContentType = when (this) {
    DomainLikedContent.ContentType.Album -> UiLikedContent.ContentType.Album
    DomainLikedContent.ContentType.Artist -> UiLikedContent.ContentType.Artist
    DomainLikedContent.ContentType.Track -> UiLikedContent.ContentType.Track
}

private fun List<DomainLikedContent>.toUi(): List<UiLikedContent> = map { it.toUi() }

private fun DomainPlaybackPlaylistContext.toUi(): UiPlaybackPlaylistContext = when (this) {
    is DomainPlaybackPlaylistContext.Album -> UiPlaybackPlaylistContext.Album(albumId)
    is DomainPlaybackPlaylistContext.UserPlaylist -> UiPlaybackPlaylistContext.UserPlaylist(userPlaylistId, isEdited)
    DomainPlaybackPlaylistContext.UserMix -> UiPlaybackPlaylistContext.UserMix
}

private fun DomainPlaybackPlaylist.toUi(): UiPlaybackPlaylist = UiPlaybackPlaylist(
    context = context.toUi(),
    tracksIds = tracksIds
)

private fun DomainConnectionState.toUi(): UiConnectionState = when (this) {
    is DomainConnectionState.Connected -> UiConnectionState.Connected(deviceId, serverVersion)
    DomainConnectionState.Connecting -> UiConnectionState.Connecting
    DomainConnectionState.Disconnected -> UiConnectionState.Disconnected
    is DomainConnectionState.Error -> UiConnectionState.Error(message)
}

private fun DomainPermission.toUi(): UiPermission? = when (this) {
    DomainPermission.AccessCatalog -> UiPermission.AccessCatalog
    DomainPermission.LikeContent -> UiPermission.LikeContent
    DomainPermission.OwnPlaylists -> UiPermission.OwnPlaylists
    DomainPermission.EditCatalog -> UiPermission.EditCatalog
    DomainPermission.ManagePermissions -> UiPermission.ManagePermissions
    DomainPermission.IssueContentDownload -> UiPermission.IssueContentDownload
    DomainPermission.RebootServer -> UiPermission.RebootServer
    DomainPermission.ViewAnalytics -> UiPermission.ViewAnalytics
}