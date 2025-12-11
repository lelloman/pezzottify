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
import com.lelloman.pezzottify.android.domain.settings.ThemeMode as DomainThemeMode
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.storage.StorageInfo as DomainStorageInfo
import com.lelloman.pezzottify.android.domain.storage.StoragePressureLevel as DomainStoragePressureLevel
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent as DomainLikedContent
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist as DomainPlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext as DomainPlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.settings.usecase.UpdateDirectDownloadsSetting
import com.lelloman.pezzottify.android.domain.settings.usecase.UpdateExternalSearchSetting
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily as UiAppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette as UiColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode as UiThemeMode
import com.lelloman.pezzottify.android.ui.model.StorageInfo as UiStorageInfo
import com.lelloman.pezzottify.android.ui.model.StoragePressureLevel as UiStoragePressureLevel
import com.lelloman.pezzottify.android.ui.model.LikedContent as UiLikedContent
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylist as UiPlaybackPlaylist
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylistContext as UiPlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.statics.usecase.GetPopularContent
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.domain.user.GetRecentlyViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.GetSearchHistoryEntriesUseCase
import com.lelloman.pezzottify.android.domain.user.LogSearchHistoryEntryUseCase
import com.lelloman.pezzottify.android.domain.user.LogViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.usercontent.GetLikedStateUseCase
import com.lelloman.pezzottify.android.domain.usercontent.ToggleLikeUseCase
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import java.util.UUID
import kotlinx.coroutines.flow.first
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.logging.LogFileManager
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.track.TrackScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenState
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.PopularAlbumState
import com.lelloman.pezzottify.android.ui.screen.main.home.PopularArtistState
import com.lelloman.pezzottify.android.ui.screen.main.home.PopularContentState
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist.UiUserPlaylistDetails
import com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist.UserPlaylistScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.library.LibraryScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings.StyleSettingsViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.settings.SettingsScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.settings.logviewer.LogViewerScreenViewModel
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
        override fun getInitialHost(): String = configStore.baseUrl.value

        override fun getInitialEmail(): String =
            authStore.getLastUsedHandle() ?: ""

        override suspend fun setHost(host: String): LoginViewModel.Interactor.SetHostResult =
            when (configStore.setBaseUrl(host)) {
                ConfigStore.SetBaseUrlResult.Success -> LoginViewModel.Interactor.SetHostResult.Success
                ConfigStore.SetBaseUrlResult.InvalidUrl -> LoginViewModel.Interactor.SetHostResult.InvalidUrl
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
        updateExternalSearchSetting: UpdateExternalSearchSetting,
        logFileManager: LogFileManager,
        configStore: ConfigStore,
    ): SettingsScreenViewModel.Interactor = object : SettingsScreenViewModel.Interactor {
        override fun getThemeMode(): UiThemeMode = userSettingsStore.themeMode.value.toUi()

        override fun getColorPalette(): UiColorPalette = userSettingsStore.colorPalette.value.toUi()

        override fun getFontFamily(): UiAppFontFamily = userSettingsStore.fontFamily.value.toUi()

        override fun isCacheEnabled(): Boolean = userSettingsStore.isInMemoryCacheEnabled.value

        override fun getStorageInfo(): UiStorageInfo = storageMonitor.storageInfo.value.toUi()

        override fun isDirectDownloadsEnabled(): Boolean = userSettingsStore.directDownloadsEnabled.value

        override fun hasIssueContentDownloadPermission(): Boolean =
            permissionsStore.permissions.value.contains(DomainPermission.IssueContentDownload)

        override fun isExternalSearchEnabled(): Boolean = userSettingsStore.isExternalSearchEnabled.value

        override fun hasRequestContentPermission(): Boolean =
            permissionsStore.permissions.value.contains(DomainPermission.RequestContent)

        override fun observeThemeMode(): Flow<UiThemeMode> = userSettingsStore.themeMode.map { it.toUi()}

        override fun observeColorPalette(): Flow<UiColorPalette>  = userSettingsStore.colorPalette.map { it.toUi() }

        override fun observeFontFamily(): Flow<UiAppFontFamily> = userSettingsStore.fontFamily.map { it.toUi() }

        override fun observeCacheEnabled() = userSettingsStore.isInMemoryCacheEnabled

        override fun observeStorageInfo(): Flow<UiStorageInfo> = storageMonitor.storageInfo.map { it.toUi() }

        override fun observeDirectDownloadsEnabled(): Flow<Boolean> = userSettingsStore.directDownloadsEnabled

        override fun observeHasIssueContentDownloadPermission(): Flow<Boolean> =
            permissionsStore.permissions.map { it.contains(DomainPermission.IssueContentDownload) }

        override fun observeExternalSearchEnabled(): Flow<Boolean> = userSettingsStore.isExternalSearchEnabled

        override fun observeHasRequestContentPermission(): Flow<Boolean> =
            permissionsStore.permissions.map { it.contains(DomainPermission.RequestContent) }

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

        override suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean {
            updateDirectDownloadsSetting(enabled)
            return true // Setting is saved locally and synced in background
        }

        override suspend fun setExternalSearchEnabled(enabled: Boolean) {
            updateExternalSearchSetting(enabled)
        }

        override fun observeFileLoggingEnabled(): Flow<Boolean> = userSettingsStore.isFileLoggingEnabled

        override suspend fun setFileLoggingEnabled(enabled: Boolean) {
            userSettingsStore.setFileLoggingEnabled(enabled)
        }

        override fun isFileLoggingEnabled(): Boolean = userSettingsStore.isFileLoggingEnabled.value

        override fun hasLogFiles(): Boolean = logFileManager.hasLogs()

        override fun getLogFilesSize(): String = logFileManager.getFormattedLogSize()

        override fun getShareLogsIntent(): android.content.Intent = logFileManager.createShareIntent()

        override fun clearLogs() = logFileManager.clearLogs()

        override fun getBaseUrl(): String = configStore.baseUrl.value

        override suspend fun setBaseUrl(url: String): SettingsScreenViewModel.SetBaseUrlResult =
            when (configStore.setBaseUrl(url)) {
                ConfigStore.SetBaseUrlResult.Success -> SettingsScreenViewModel.SetBaseUrlResult.Success
                ConfigStore.SetBaseUrlResult.InvalidUrl -> SettingsScreenViewModel.SetBaseUrlResult.InvalidUrl
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
    fun provideLogViewerScreenInteractor(
        logFileManager: LogFileManager,
    ): LogViewerScreenViewModel.Interactor = object : LogViewerScreenViewModel.Interactor {
        override fun getLogContent(): String = logFileManager.getLogContent()
    }

    @Provides
    fun provideSearchScreenInteractor(
        performSearch: PerformSearch,
        loggerFactory: LoggerFactory,
        getRecentlyViewedContent: GetRecentlyViewedContentUseCase,
        getSearchHistoryEntries: GetSearchHistoryEntriesUseCase,
        logSearchHistoryEntry: LogSearchHistoryEntryUseCase,
        userSettingsStore: UserSettingsStore,
        permissionsStore: PermissionsStore,
        performExternalSearchUseCase: com.lelloman.pezzottify.android.domain.download.PerformExternalSearchUseCase,
        getDownloadLimitsUseCase: com.lelloman.pezzottify.android.domain.download.GetDownloadLimitsUseCase,
        requestAlbumDownloadUseCase: com.lelloman.pezzottify.android.domain.download.RequestAlbumDownloadUseCase,
    ): SearchScreenViewModel.Interactor =
        object : SearchScreenViewModel.Interactor {
            private val logger = loggerFactory.getLogger("SearchScreenViewModel.Interactor")

            override suspend fun search(
                query: String,
                filters: List<SearchScreenViewModel.InteractorSearchFilter>?
            ): Result<List<Pair<String, SearchScreenViewModel.SearchedItemType>>> {
                logger.debug("search($query, filters=$filters)")
                val domainFilters = filters?.map { filter ->
                    when (filter) {
                        SearchScreenViewModel.InteractorSearchFilter.Album -> RemoteApiClient.SearchFilter.Album
                        SearchScreenViewModel.InteractorSearchFilter.Artist -> RemoteApiClient.SearchFilter.Artist
                        SearchScreenViewModel.InteractorSearchFilter.Track -> RemoteApiClient.SearchFilter.Track
                    }
                }
                val performSearchResult = performSearch(query, domainFilters)
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

            override fun canUseExternalSearch(): Flow<Boolean> =
                userSettingsStore.isExternalSearchEnabled.combine(
                    permissionsStore.permissions
                ) { isEnabled, permissions ->
                    isEnabled && permissions.contains(DomainPermission.RequestContent)
                }

            override fun isExternalModeEnabled(): Flow<Boolean> =
                userSettingsStore.isExternalModeEnabled

            override suspend fun setExternalModeEnabled(enabled: Boolean) {
                userSettingsStore.setExternalModeEnabled(enabled)
            }

            override suspend fun externalSearch(
                query: String,
                type: SearchScreenViewModel.InteractorExternalSearchType
            ): Result<List<SearchScreenViewModel.ExternalSearchItem>> {
                logger.debug("externalSearch($query, type=$type)")
                val domainType = when (type) {
                    SearchScreenViewModel.InteractorExternalSearchType.Album ->
                        RemoteApiClient.ExternalSearchType.Album
                    SearchScreenViewModel.InteractorExternalSearchType.Artist ->
                        RemoteApiClient.ExternalSearchType.Artist
                }
                val result = performExternalSearchUseCase(query, domainType)
                return result.map { items ->
                    items.map { item ->
                        SearchScreenViewModel.ExternalSearchItem(
                            id = item.id,
                            name = item.name,
                            artistName = item.artistName,
                            year = item.year,
                            imageUrl = item.imageUrl,
                            inCatalog = item.inCatalog,
                            inQueue = item.inQueue,
                            catalogId = null, // Server doesn't return catalog ID yet
                        )
                    }
                }
            }

            override suspend fun getDownloadLimits(): Result<SearchScreenViewModel.DownloadLimitsData> {
                logger.debug("getDownloadLimits()")
                val result = getDownloadLimitsUseCase()
                return result.map { limits ->
                    SearchScreenViewModel.DownloadLimitsData(
                        requestsToday = limits.requestsToday,
                        maxPerDay = limits.maxPerDay,
                        canRequest = limits.canRequest,
                        inQueue = limits.inQueue,
                        maxQueue = limits.maxQueue,
                    )
                }
            }

            override suspend fun requestAlbumDownload(
                albumId: String,
                albumName: String,
                artistName: String
            ): Result<Unit> {
                logger.debug("requestAlbumDownload($albumId, $albumName, $artistName)")
                return requestAlbumDownloadUseCase(albumId, albumName, artistName).map { }
            }
        }

    @Provides
    fun provideAlbumScreenInteractor(
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        userPlaylistStore: UserPlaylistStore,
        staticsStore: StaticsStore,
    ): AlbumScreenViewModel.Interactor = object : AlbumScreenViewModel.Interactor {
        override fun playAlbum(albumId: String) {
            player.loadAlbum(albumId)
        }

        override fun playTrack(albumId: String, trackId: String) {
            player.loadAlbum(albumId, trackId)
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

        override fun isLiked(contentId: String): Flow<Boolean> =
            getLikedStateUseCase(contentId)

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Album, currentlyLiked)
        }

        override fun getUserPlaylists(): Flow<List<UiUserPlaylist>> =
            userPlaylistStore.getPlaylists().map { playlists ->
                playlists.map { playlist ->
                    UiUserPlaylist(
                        id = playlist.id,
                        name = playlist.name,
                        trackCount = playlist.trackIds.size,
                    )
                }
            }

        override fun playTrackDirectly(trackId: String) {
            player.loadSingleTrack(trackId)
        }

        override fun addTrackToQueue(trackId: String) {
            player.addTracksToPlaylist(listOf(trackId))
        }

        override fun addAlbumToQueue(albumId: String) {
            player.addAlbumToPlaylist(albumId)
        }

        override suspend fun addTrackToPlaylist(trackId: String, playlistId: String) {
            userPlaylistStore.addTrackToPlaylist(playlistId, trackId)
        }

        override suspend fun addAlbumToPlaylist(albumId: String, playlistId: String) {
            val album = staticsStore.getAlbum(albumId).first()
            if (album != null) {
                val trackIds = album.discs.flatMap { it.tracksIds }
                userPlaylistStore.addTracksToPlaylist(playlistId, trackIds)
            }
        }

        override suspend fun createPlaylist(name: String) {
            val id = UUID.randomUUID().toString()
            userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList())
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
    fun provideTrackScreenInteractor(
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
    ): TrackScreenViewModel.Interactor = object : TrackScreenViewModel.Interactor {
        override fun playTrack(albumId: String, trackId: String) {
            // Always play the track by loading the album starting from this track
            player.loadAlbum(albumId, trackId)
        }

        override fun logViewedTrack(trackId: String) {
            logViewedContentUseCase(trackId, ViewedContent.Type.Track)
        }

        override fun getCurrentPlayingTrackId(): Flow<String?> =
            player.playbackPlaylist.combine(player.currentTrackIndex) { playlist, currentTrackIndex ->
                if (playlist != null && currentTrackIndex != null && currentTrackIndex in playlist.tracksIds.indices) {
                    playlist.tracksIds[currentTrackIndex]
                } else {
                    null
                }
            }

        override fun isLiked(contentId: String): Flow<Boolean> =
            getLikedStateUseCase(contentId)

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Track, currentlyLiked)
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
        getPopularContentUseCase: GetPopularContent,
        authStore: AuthStore,
        webSocketManager: WebSocketManager,
        configStore: ConfigStore,
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

        override suspend fun getPopularContent(): PopularContentState? {
            // Only fetch if logged in - otherwise auth token is empty and server returns 403
            if (authStore.getAuthState().value !is AuthState.LoggedIn) {
                return null
            }
            val result = getPopularContentUseCase()
            return result.getOrNull()?.let { popularContent ->
                val baseUrl = configStore.baseUrl.value
                PopularContentState(
                    albums = popularContent.albums.map { album ->
                        PopularAlbumState(
                            id = album.id,
                            name = album.name,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, album.displayImageId),
                            artistNames = album.artistNames,
                        )
                    },
                    artists = popularContent.artists.map { artist ->
                        PopularArtistState(
                            id = artist.id,
                            name = artist.name,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, artist.displayImageId),
                        )
                    },
                )
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
        userPlaylistStore: UserPlaylistStore,
    ): LibraryScreenViewModel.Interactor =
        object : LibraryScreenViewModel.Interactor {
            override fun getLikedContent(): Flow<List<UiLikedContent>> =
                userContentStore.getLikedContent().map { it.toUi() }

            override fun getPlaylists() = userPlaylistStore.getPlaylists().map { playlists ->
                playlists.map { playlist ->
                    com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist(
                        id = playlist.id,
                        name = playlist.name,
                        trackCount = playlist.trackIds.size,
                    )
                }
            }

            override suspend fun createPlaylist(name: String) {
                val id = UUID.randomUUID().toString()
                userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList())
            }
        }

    @Provides
    fun provideUserPlaylistScreenInteractor(
        userPlaylistStore: UserPlaylistStore,
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
    ): UserPlaylistScreenViewModel.Interactor =
        object : UserPlaylistScreenViewModel.Interactor {
            override fun getPlaylist(playlistId: String): Flow<UiUserPlaylistDetails?> =
                userPlaylistStore.getPlaylist(playlistId).map { playlist ->
                    playlist?.let {
                        UiUserPlaylistDetails(
                            id = it.id,
                            name = it.name,
                            trackIds = it.trackIds,
                        )
                    }
                }

            override fun playPlaylist(playlistId: String) {
                player.loadUserPlaylist(playlistId)
            }

            override fun playTrack(playlistId: String, trackId: String) {
                // Clicking a track in playlist loads the playlist starting from that track
                player.loadUserPlaylist(playlistId, trackId)
            }

            override fun logViewedPlaylist(playlistId: String) {
                logViewedContentUseCase(playlistId, ViewedContent.Type.UserPlaylist)
            }

            override fun getCurrentPlayingTrackId(): Flow<String?> =
                player.playbackPlaylist.combine(player.currentTrackIndex) { playlist, currentTrackIndex ->
                    if (playlist != null && currentTrackIndex != null && currentTrackIndex in playlist.tracksIds.indices) {
                        playlist.tracksIds[currentTrackIndex]
                    } else {
                        null
                    }
                }

            override fun getUserPlaylists(): Flow<List<UiUserPlaylist>> =
                userPlaylistStore.getPlaylists().map { playlists ->
                    playlists.map { playlist ->
                        UiUserPlaylist(
                            id = playlist.id,
                            name = playlist.name,
                            trackCount = playlist.trackIds.size,
                        )
                    }
                }

            override fun playTrackDirectly(trackId: String) {
                player.loadSingleTrack(trackId)
            }

            override fun addTrackToQueue(trackId: String) {
                player.addTracksToPlaylist(listOf(trackId))
            }

            override fun addPlaylistToQueue(playlistId: String) {
                player.addUserPlaylistToQueue(playlistId)
            }

            override suspend fun addTrackToPlaylist(trackId: String, playlistId: String) {
                userPlaylistStore.addTrackToPlaylist(playlistId, trackId)
            }

            override suspend fun removeTrackFromPlaylist(playlistId: String, trackId: String) {
                userPlaylistStore.removeTrackFromPlaylist(playlistId, trackId)
            }

            override suspend fun createPlaylist(name: String) {
                val id = UUID.randomUUID().toString()
                userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList())
            }
        }

    @Provides
    fun provideMyRequestsScreenInteractor(
        getMyDownloadRequestsUseCase: com.lelloman.pezzottify.android.domain.download.GetMyDownloadRequestsUseCase,
        getDownloadLimitsUseCase: com.lelloman.pezzottify.android.domain.download.GetDownloadLimitsUseCase,
    ): com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.Interactor =
        object : com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.Interactor {
            override suspend fun getMyRequests(): Result<List<com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadRequest>> {
                val result = getMyDownloadRequestsUseCase()
                return result.map { response ->
                    response.requests.map { request ->
                        com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadRequest(
                            id = request.id,
                            albumName = request.contentName,
                            artistName = request.artistName ?: "",
                            status = when (request.status) {
                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Pending ->
                                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Pending
                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.InProgress ->
                                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.InProgress
                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.RetryWaiting ->
                                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Pending
                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Completed ->
                                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Completed
                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Failed ->
                                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Failed
                            },
                            progress = request.progress?.let { progress ->
                                com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestProgress(
                                    current = progress.completed,
                                    total = progress.totalChildren,
                                )
                            },
                            errorMessage = request.errorMessage,
                            catalogId = request.contentId,
                            createdAt = request.createdAt,
                            completedAt = request.completedAt,
                            queuePosition = request.queuePosition,
                        )
                    }
                }
            }

            override suspend fun getDownloadLimits(): Result<com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.DownloadLimitsData> {
                val result = getDownloadLimitsUseCase()
                return result.map { limits ->
                    com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.DownloadLimitsData(
                        requestsToday = limits.requestsToday,
                        maxPerDay = limits.maxPerDay,
                        inQueue = limits.inQueue,
                        maxQueue = limits.maxQueue,
                    )
                }
            }
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
    DomainPermission.ServerAdmin -> UiPermission.ServerAdmin
    DomainPermission.ViewAnalytics -> UiPermission.ViewAnalytics
    DomainPermission.RequestContent -> UiPermission.RequestContent
    DomainPermission.DownloadManagerAdmin -> UiPermission.DownloadManagerAdmin
}