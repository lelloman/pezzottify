package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.usecase.IsLoggedIn
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogin
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogout
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.notifications.getAlbumId
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
import com.lelloman.pezzottify.android.domain.settings.usecase.UpdateExternalSearchSetting
import com.lelloman.pezzottify.android.domain.settings.usecase.UpdateNotifyWhatsNewSetting
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
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWhatsNew
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.domain.user.GetRecentlyViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.GetSearchHistoryEntriesUseCase
import com.lelloman.pezzottify.android.domain.user.LogSearchHistoryEntryUseCase
import com.lelloman.pezzottify.android.domain.user.LogViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.usercontent.GetLikedStateUseCase
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSynchronizer
import com.lelloman.pezzottify.android.domain.usercontent.ToggleLikeUseCase
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import java.util.UUID
import kotlinx.coroutines.flow.first
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.logging.LogFileManager
import com.lelloman.pezzottify.android.ui.screen.about.AboutScreenViewModel
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
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryErrorType
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryException
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.UiListeningEvent
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings.StyleSettingsViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.settings.SettingsScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.whatsnew.WhatsNewScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.notifications.NotificationListScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.notifications.UiNotification
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
    fun provideAboutScreenInteractor(
        buildInfo: BuildInfo,
        configStore: ConfigStore,
        skeletonStore: com.lelloman.pezzottify.android.domain.skeleton.SkeletonStore,
        webSocketManager: WebSocketManager,
    ): AboutScreenViewModel.Interactor = object : AboutScreenViewModel.Interactor {
        override fun getVersionName(): String = buildInfo.versionName

        override fun getGitCommit(): String = buildInfo.gitCommit

        override fun getServerUrl(): String = configStore.baseUrl.value

        override fun observeServerVersion() = webSocketManager.connectionState.map { state ->
            when (state) {
                is DomainConnectionState.Connected -> state.serverVersion
                else -> "disconnected"
            }
        }

        override suspend fun getSkeletonCounts(): AboutScreenViewModel.SkeletonCountsData {
            val counts = skeletonStore.getCounts()
            return AboutScreenViewModel.SkeletonCountsData(
                artists = counts.artists,
                albums = counts.albums,
                tracks = counts.tracks,
            )
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
        updateExternalSearchSetting: UpdateExternalSearchSetting,
        updateNotifyWhatsNewSetting: UpdateNotifyWhatsNewSetting,
        logFileManager: LogFileManager,
        configStore: ConfigStore,
        catalogSkeletonSyncer: com.lelloman.pezzottify.android.domain.skeleton.CatalogSkeletonSyncer,
    ): SettingsScreenViewModel.Interactor = object : SettingsScreenViewModel.Interactor {
        override fun getThemeMode(): UiThemeMode = userSettingsStore.themeMode.value.toUi()

        override fun getColorPalette(): UiColorPalette = userSettingsStore.colorPalette.value.toUi()

        override fun getFontFamily(): UiAppFontFamily = userSettingsStore.fontFamily.value.toUi()

        override fun isCacheEnabled(): Boolean = userSettingsStore.isInMemoryCacheEnabled.value

        override fun getStorageInfo(): UiStorageInfo = storageMonitor.storageInfo.value.toUi()

        override fun isExternalSearchEnabled(): Boolean = userSettingsStore.isExternalSearchEnabled.value

        override fun hasRequestContentPermission(): Boolean =
            permissionsStore.permissions.value.contains(DomainPermission.RequestContent)

        override fun observeThemeMode(): Flow<UiThemeMode> = userSettingsStore.themeMode.map { it.toUi()}

        override fun observeColorPalette(): Flow<UiColorPalette>  = userSettingsStore.colorPalette.map { it.toUi() }

        override fun observeFontFamily(): Flow<UiAppFontFamily> = userSettingsStore.fontFamily.map { it.toUi() }

        override fun observeCacheEnabled() = userSettingsStore.isInMemoryCacheEnabled

        override fun observeStorageInfo(): Flow<UiStorageInfo> = storageMonitor.storageInfo.map { it.toUi() }

        override fun observeExternalSearchEnabled(): Flow<Boolean> = userSettingsStore.isExternalSearchEnabled

        override fun observeHasRequestContentPermission(): Flow<Boolean> =
            permissionsStore.permissions.map { it.contains(DomainPermission.RequestContent) }

        override fun isNotifyWhatsNewEnabled(): Boolean = userSettingsStore.isNotifyWhatsNewEnabled.value

        override fun observeNotifyWhatsNewEnabled(): Flow<Boolean> = userSettingsStore.isNotifyWhatsNewEnabled

        override suspend fun setNotifyWhatsNewEnabled(enabled: Boolean) {
            updateNotifyWhatsNewSetting(enabled)
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

        override suspend fun forceSkeletonResync(): com.lelloman.pezzottify.android.ui.screen.main.settings.SkeletonResyncResult {
            return when (val result = catalogSkeletonSyncer.forceFullSync()) {
                is com.lelloman.pezzottify.android.domain.skeleton.CatalogSkeletonSyncer.SyncResult.Success ->
                    com.lelloman.pezzottify.android.ui.screen.main.settings.SkeletonResyncResult.Success
                is com.lelloman.pezzottify.android.domain.skeleton.CatalogSkeletonSyncer.SyncResult.AlreadyUpToDate ->
                    com.lelloman.pezzottify.android.ui.screen.main.settings.SkeletonResyncResult.AlreadyUpToDate
                is com.lelloman.pezzottify.android.domain.skeleton.CatalogSkeletonSyncer.SyncResult.Failed ->
                    com.lelloman.pezzottify.android.ui.screen.main.settings.SkeletonResyncResult.Failed(result.error)
            }
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
        getWhatsNew: GetWhatsNew,
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
                    SearchScreenViewModel.InteractorExternalSearchType.Track ->
                        RemoteApiClient.ExternalSearchType.Track
                }
                val result = performExternalSearchUseCase(query, domainType)
                return result.map { items ->
                    items.map { item ->
                        SearchScreenViewModel.ExternalSearchItem(
                            id = item.id,
                            name = item.name,
                            artistName = item.artistName,
                            albumName = item.albumName,
                            year = item.year,
                            duration = item.durationMs,
                            imageUrl = item.imageUrl,
                            inCatalog = item.inCatalog,
                            inQueue = item.inQueue,
                            catalogId = null, // Server doesn't return catalog ID yet
                            score = item.score,
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

            override suspend fun getWhatsNew(limit: Int): Result<List<SearchScreenViewModel.WhatsNewBatchData>> {
                logger.debug("getWhatsNew(limit=$limit)")
                return getWhatsNew(limit).map { response ->
                    response.batches.map { batch ->
                        SearchScreenViewModel.WhatsNewBatchData(
                            batchId = batch.id,
                            batchName = batch.name,
                            closedAt = batch.closedAt,
                            addedAlbumIds = batch.summary.albums.added.map { it.id },
                        )
                    }
                }
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
        playlistSynchronizer: PlaylistSynchronizer,
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

        override fun toggleTrackLike(trackId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(trackId, DomainLikedContent.ContentType.Track, currentlyLiked)
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
            playlistSynchronizer.wakeUp()
        }

        override suspend fun addAlbumToPlaylist(albumId: String, playlistId: String) {
            val album = staticsStore.getAlbum(albumId).first()
            if (album != null) {
                val trackIds = album.discs.flatMap { it.tracksIds }
                userPlaylistStore.addTracksToPlaylist(playlistId, trackIds)
                playlistSynchronizer.wakeUp()
            }
        }

        override suspend fun createPlaylist(name: String) {
            val id = UUID.randomUUID().toString()
            userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList(), PlaylistSyncStatus.PendingCreate)
            playlistSynchronizer.wakeUp()
        }
    }

    @Provides
    fun provideArtistScreenInteractor(
        logViewedContentUseCase: LogViewedContentUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        userSettingsStore: UserSettingsStore,
        permissionsStore: PermissionsStore,
        getExternalArtistDiscographyUseCase: com.lelloman.pezzottify.android.domain.download.GetExternalArtistDiscographyUseCase,
    ): ArtistScreenViewModel.Interactor = object : ArtistScreenViewModel.Interactor {
        override fun logViewedArtist(artistId: String) {
            logViewedContentUseCase(artistId, ViewedContent.Type.Artist)
        }

        override fun isLiked(contentId: String): Flow<Boolean> =
            getLikedStateUseCase(contentId)

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Artist, currentlyLiked)
        }

        override suspend fun canShowExternalAlbums(): Boolean {
            val hasPermission = permissionsStore.permissions.value.contains(DomainPermission.RequestContent)
            val isEnabled = userSettingsStore.isExternalSearchEnabled.value
            return hasPermission && isEnabled
        }

        override suspend fun getExternalDiscography(artistId: String): Result<List<com.lelloman.pezzottify.android.ui.screen.main.content.artist.UiExternalAlbumItem>> {
            val result = getExternalArtistDiscographyUseCase(artistId)
            return result.map { discography ->
                // Filter to only show albums not in catalog
                discography.albums
                    .filter { !it.inCatalog }
                    .map { album ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.artist.UiExternalAlbumItem(
                            id = album.id,
                            name = album.name,
                            imageUrl = album.imageUrl,
                            year = album.year,
                            inQueue = album.inQueue,
                        )
                    }
            }
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
        player: PezzottifyPlayer,
        notificationRepository: NotificationRepository,
        playbackMetadataProvider: com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider,
    ): MainScreenViewModel.Interactor =
        object : MainScreenViewModel.Interactor {

            val logger = loggerFactory.getLogger(MainScreenViewModel.Interactor::class)

            override fun getNotificationUnreadCount(): Flow<Int> =
                notificationRepository.unreadCount

            override fun getPlaybackState(): Flow<MainScreenViewModel.Interactor.PlaybackState?> =
                playbackMetadataProvider.queueState
                    .combine(player.isPlaying) { queueState, isPlaying -> queueState to isPlaying }
                    .combine(player.currentTrackPercent) { (queueState, isPlaying), trackPercent ->
                        logger.debug("Combining queueState + isPlaying + trackPercent: ${queueState?.currentTrack?.trackName} - $isPlaying - $trackPercent")
                        val currentTrack = queueState?.currentTrack
                        if (currentTrack != null) {
                            val currentIndex = queueState.currentIndex
                            val nextTrack = queueState.tracks.getOrNull(currentIndex + 1)
                            val previousTrack = queueState.tracks.getOrNull(currentIndex - 1)

                            MainScreenViewModel.Interactor.PlaybackState.Loaded(
                                isPlaying = isPlaying,
                                trackId = currentTrack.trackId,
                                trackName = currentTrack.trackName,
                                albumName = currentTrack.albumName,
                                albumImageUrl = currentTrack.artworkUrl,
                                artists = currentTrack.artistNames.mapIndexed { index, name ->
                                    com.lelloman.pezzottify.android.ui.content.ArtistInfo(
                                        id = index.toString(), // We don't have artist IDs in metadata
                                        name = name,
                                    )
                                },
                                trackPercent = trackPercent ?: 0f,
                                nextTrackName = nextTrack?.trackName,
                                nextTrackArtists = nextTrack?.artistNames?.mapIndexed { index, name ->
                                    com.lelloman.pezzottify.android.ui.content.ArtistInfo(
                                        id = index.toString(),
                                        name = name,
                                    )
                                } ?: emptyList(),
                                previousTrackName = previousTrack?.trackName,
                                previousTrackArtists = previousTrack?.artistNames?.mapIndexed { index, name ->
                                    com.lelloman.pezzottify.android.ui.content.ArtistInfo(
                                        id = index.toString(),
                                        name = name,
                                    )
                                } ?: emptyList(),
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
        player: PezzottifyPlayer,
        playbackMetadataProvider: com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider,
    ): PlayerScreenViewModel.Interactor =
        object : PlayerScreenViewModel.Interactor {
            override fun getPlaybackState(): Flow<PlayerScreenViewModel.Interactor.PlaybackState?> =
                playbackMetadataProvider.queueState
                    .combine(player.isPlaying) { queueState, isPlaying -> queueState to isPlaying }
                    .combine(player.currentTrackPercent) { (queueState, isPlaying), trackPercent ->
                        Triple(queueState, isPlaying, trackPercent)
                    }
                    .combine(player.currentTrackProgressSec) { (queueState, isPlaying, trackPercent), progressSec ->
                        data class PlaybackData(
                            val queueState: com.lelloman.pezzottify.android.domain.player.PlaybackQueueState?,
                            val isPlaying: Boolean,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                        )
                        PlaybackData(queueState, isPlaying, trackPercent, progressSec)
                    }
                    .combine(player.volumeState) { data, volumeState ->
                        data class PlaybackData2(
                            val queueState: com.lelloman.pezzottify.android.domain.player.PlaybackQueueState?,
                            val isPlaying: Boolean,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                            val volume: Float,
                            val isMuted: Boolean,
                        )
                        PlaybackData2(data.queueState, data.isPlaying, data.trackPercent, data.progressSec, volumeState.volume, volumeState.isMuted)
                    }
                    .combine(player.shuffleEnabled) { data, shuffleEnabled ->
                        data class PlaybackData3(
                            val queueState: com.lelloman.pezzottify.android.domain.player.PlaybackQueueState?,
                            val isPlaying: Boolean,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                            val volume: Float,
                            val isMuted: Boolean,
                            val shuffleEnabled: Boolean,
                        )
                        PlaybackData3(data.queueState, data.isPlaying, data.trackPercent, data.progressSec, data.volume, data.isMuted, shuffleEnabled)
                    }
                    .combine(player.repeatMode) { data, repeatMode ->
                        val currentTrack = data.queueState?.currentTrack
                        if (currentTrack != null) {
                            val currentIndex = data.queueState.currentIndex
                            val hasNext = currentIndex < data.queueState.tracks.lastIndex
                            val hasPrevious = currentIndex > 0
                            val repeatModeUi = when (repeatMode) {
                                RepeatMode.OFF -> RepeatModeUi.OFF
                                RepeatMode.ALL -> RepeatModeUi.ALL
                                RepeatMode.ONE -> RepeatModeUi.ONE
                            }
                            PlayerScreenViewModel.Interactor.PlaybackState.Loaded(
                                isPlaying = data.isPlaying,
                                trackId = currentTrack.trackId,
                                trackName = currentTrack.trackName,
                                albumId = currentTrack.albumId,
                                albumName = currentTrack.albumName,
                                albumImageUrl = currentTrack.artworkUrl,
                                artists = currentTrack.artistNames.mapIndexed { index, name ->
                                    com.lelloman.pezzottify.android.ui.content.ArtistInfo(
                                        id = index.toString(),
                                        name = name,
                                    )
                                },
                                trackPercent = data.trackPercent ?: 0f,
                                trackProgressSec = data.progressSec ?: 0,
                                trackDurationSec = currentTrack.durationSeconds,
                                hasNextTrack = hasNext,
                                hasPreviousTrack = hasPrevious,
                                volume = data.volume,
                                isMuted = data.isMuted,
                                shuffleEnabled = data.shuffleEnabled,
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
        player: PezzottifyPlayer,
        playbackMetadataProvider: com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider,
        userPlaylistStore: UserPlaylistStore,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        playlistSynchronizer: PlaylistSynchronizer,
    ): QueueScreenViewModel.Interactor =
        object : QueueScreenViewModel.Interactor {
            override fun getQueueState(): Flow<QueueScreenViewModel.Interactor.QueueState?> =
                playbackMetadataProvider.queueState
                    .combine(player.playbackPlaylist) { queueState, playlist ->
                        if (queueState != null && playlist != null) {
                            val playlistContext = playlist.context
                            val (contextType, contextName, canSave) = when (playlistContext) {
                                is com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext.Album -> Triple(
                                    com.lelloman.pezzottify.android.ui.screen.queue.QueueContextType.Album,
                                    playlistContext.albumId,
                                    false
                                )
                                is com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext.UserPlaylist -> Triple(
                                    com.lelloman.pezzottify.android.ui.screen.queue.QueueContextType.UserPlaylist,
                                    playlistContext.userPlaylistId,
                                    playlistContext.isEdited
                                )
                                is com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext.UserMix -> Triple(
                                    com.lelloman.pezzottify.android.ui.screen.queue.QueueContextType.UserMix,
                                    "user_mix",
                                    true
                                )
                            }
                            QueueScreenViewModel.Interactor.QueueState(
                                tracks = queueState.tracks.map { track ->
                                    QueueScreenViewModel.Interactor.QueueTrack(
                                        trackId = track.trackId,
                                        trackName = track.trackName,
                                        albumId = track.albumId,
                                        artists = track.artistNames.mapIndexed { index, name ->
                                            com.lelloman.pezzottify.android.ui.content.ArtistInfo(
                                                id = index.toString(),
                                                name = name,
                                            )
                                        },
                                        durationSeconds = track.durationSeconds,
                                    )
                                },
                                currentIndex = queueState.currentIndex,
                                contextName = contextName,
                                contextType = contextType,
                                canSaveAsPlaylist = canSave,
                            )
                        } else {
                            null
                        }
                    }

            override fun playTrackAtIndex(index: Int) = player.loadTrackIndex(index)

            override fun moveTrack(fromIndex: Int, toIndex: Int) =
                player.moveTrack(fromIndex, toIndex)

            override fun removeTrack(trackId: String) = player.removeTrackFromPlaylist(trackId)

            override fun playTrackDirectly(trackId: String) = player.loadSingleTrack(trackId)

            override fun addTrackToQueue(trackId: String) = player.addTracksToPlaylist(listOf(trackId))

            override suspend fun addTrackToPlaylist(trackId: String, playlistId: String) {
                userPlaylistStore.addTrackToPlaylist(playlistId, trackId)
                playlistSynchronizer.wakeUp()
            }

            override suspend fun createPlaylist(name: String) {
                val id = UUID.randomUUID().toString()
                userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList(), PlaylistSyncStatus.PendingCreate)
                playlistSynchronizer.wakeUp()
            }

            override fun toggleLike(trackId: String, currentlyLiked: Boolean) {
                toggleLikeUseCase(trackId, DomainLikedContent.ContentType.Track, currentlyLiked)
            }

            override fun isLiked(trackId: String): Flow<Boolean> = getLikedStateUseCase(trackId)

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
        }

    @Provides
    fun provideLibraryScreenInteractor(
        userContentStore: UserContentStore,
        userPlaylistStore: UserPlaylistStore,
        playlistSynchronizer: PlaylistSynchronizer,
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
                userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList(), PlaylistSyncStatus.PendingCreate)
                playlistSynchronizer.wakeUp()
            }
        }

    @Provides
    fun provideUserPlaylistScreenInteractor(
        userPlaylistStore: UserPlaylistStore,
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        playlistSynchronizer: PlaylistSynchronizer,
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
                playlistSynchronizer.wakeUp()
            }

            override suspend fun removeTrackFromPlaylist(playlistId: String, trackId: String) {
                userPlaylistStore.removeTrackFromPlaylist(playlistId, trackId)
                playlistSynchronizer.wakeUp()
            }

            override suspend fun createPlaylist(name: String) {
                val id = UUID.randomUUID().toString()
                userPlaylistStore.createOrUpdatePlaylist(id, name, emptyList(), PlaylistSyncStatus.PendingCreate)
                playlistSynchronizer.wakeUp()
            }

            override fun isLiked(contentId: String): Flow<Boolean> =
                getLikedStateUseCase(contentId)

            override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
                toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Track, currentlyLiked)
            }
        }

    @Provides
    fun provideMyRequestsScreenInteractor(
        getMyDownloadRequestsUseCase: com.lelloman.pezzottify.android.domain.download.GetMyDownloadRequestsUseCase,
        getDownloadLimitsUseCase: com.lelloman.pezzottify.android.domain.download.GetDownloadLimitsUseCase,
        downloadStatusRepository: com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository,
    ): com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.Interactor =
        object : com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.Interactor {
            override suspend fun getMyRequests(limit: Int, offset: Int): Result<List<com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadRequest>> {
                val result = getMyDownloadRequestsUseCase(limit, offset)
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

            override fun observeUpdates(): kotlinx.coroutines.flow.Flow<com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadStatusUpdate> {
                return downloadStatusRepository.observeAllUpdates().map { update ->
                    when (update) {
                        is com.lelloman.pezzottify.android.domain.download.DownloadStatusUpdate.Created ->
                            com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadStatusUpdate.Created(
                                requestId = update.requestId,
                                contentId = update.contentId,
                                contentName = update.contentName,
                                artistName = update.artistName,
                                queuePosition = update.queuePosition,
                            )
                        is com.lelloman.pezzottify.android.domain.download.DownloadStatusUpdate.StatusChanged ->
                            com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadStatusUpdate.StatusChanged(
                                requestId = update.requestId,
                                status = when (update.status) {
                                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Pending ->
                                        com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Pending
                                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.InProgress ->
                                        com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.InProgress
                                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Completed ->
                                        com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Completed
                                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Failed ->
                                        com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Failed
                                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.RetryWaiting ->
                                        com.lelloman.pezzottify.android.ui.screen.main.myrequests.RequestStatus.Pending
                                },
                                queuePosition = update.queuePosition,
                                errorMessage = update.errorMessage,
                            )
                        is com.lelloman.pezzottify.android.domain.download.DownloadStatusUpdate.ProgressUpdated ->
                            com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadStatusUpdate.ProgressUpdated(
                                requestId = update.requestId,
                                completed = update.progress.completed,
                                total = update.progress.totalChildren,
                            )
                        is com.lelloman.pezzottify.android.domain.download.DownloadStatusUpdate.Completed ->
                            com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadStatusUpdate.Completed(
                                requestId = update.requestId,
                            )
                    }
                }
            }
        }

    @Provides
    fun provideExternalAlbumScreenInteractor(
        getExternalAlbumDetailsUseCase: com.lelloman.pezzottify.android.domain.download.GetExternalAlbumDetailsUseCase,
        requestAlbumDownloadUseCase: com.lelloman.pezzottify.android.domain.download.RequestAlbumDownloadUseCase,
        downloadStatusRepository: com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository,
    ): com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.ExternalAlbumScreenViewModel.Interactor =
        object : com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.ExternalAlbumScreenViewModel.Interactor {
            override suspend fun getExternalAlbumDetails(albumId: String): Result<com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiExternalAlbumWithStatus> {
                val result = getExternalAlbumDetailsUseCase(albumId)
                return result.map { album ->
                    com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiExternalAlbumWithStatus(
                        id = album.id,
                        name = album.name,
                        artistId = album.artistId,
                        artistName = album.artistName,
                        imageUrl = album.imageUrl,
                        year = album.year,
                        albumType = album.albumType,
                        totalTracks = album.totalTracks,
                        tracks = album.tracks.map { track ->
                            com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiExternalTrack(
                                id = track.id,
                                name = track.name,
                                trackNumber = track.trackNumber,
                                durationMs = track.durationMs,
                            )
                        },
                        inCatalog = album.inCatalog,
                        requestStatus = album.requestStatus?.toUi(),
                    )
                }
            }

            override suspend fun requestAlbumDownload(
                albumId: String,
                albumName: String,
                artistName: String,
            ): Result<com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiRequestStatus> {
                val result = requestAlbumDownloadUseCase(albumId, albumName, artistName)
                return result.map { response ->
                    com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiRequestStatus(
                        requestId = response.requestId,
                        status = com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus.Pending,
                        queuePosition = response.queuePosition,
                        progress = null,
                        errorMessage = null,
                    )
                }
            }

            override fun observeDownloadStatus(albumId: String): Flow<com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiRequestStatus?> =
                downloadStatusRepository.observeStatus(albumId).map { status ->
                    status?.toUi()
                }

            private fun com.lelloman.pezzottify.android.domain.remoteapi.response.RequestStatusInfo.toUi(): com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiRequestStatus =
                com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiRequestStatus(
                    requestId = requestId,
                    status = status.toUi(),
                    queuePosition = queuePosition,
                    progress = progress?.let { p ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadProgress(
                            completed = p.completed,
                            total = p.totalChildren,
                        )
                    },
                    errorMessage = errorMessage,
                )

            private fun com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.toUi(): com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus =
                when (this) {
                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Pending ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus.Pending
                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.InProgress ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus.InProgress
                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.RetryWaiting ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus.RetryWaiting
                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Completed ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus.Completed
                    com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Failed ->
                        com.lelloman.pezzottify.android.ui.screen.main.content.externalalbum.UiDownloadStatus.Failed
                }
        }

    @Provides
    fun provideListeningHistoryScreenInteractor(
        remoteApiClient: RemoteApiClient,
    ): ListeningHistoryScreenViewModel.Interactor =
        object : ListeningHistoryScreenViewModel.Interactor {
            override suspend fun getListeningEvents(
                limit: Int,
                offset: Int,
            ): Result<List<UiListeningEvent>> {
                val response = remoteApiClient.getListeningEvents(
                    limit = limit,
                    offset = offset,
                )
                return when (response) {
                    is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Success -> {
                        Result.success(
                            response.data.map { event ->
                                UiListeningEvent(
                                    id = event.id,
                                    trackId = event.trackId,
                                    startedAt = event.startedAt,
                                    durationSeconds = event.durationSeconds,
                                    trackDurationSeconds = event.trackDurationSeconds,
                                    completed = event.completed,
                                    playbackContext = event.playbackContext,
                                    clientType = event.clientType,
                                )
                            }
                        )
                    }
                    is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Error -> {
                        val errorType = when (response) {
                            is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Error.Network ->
                                ListeningHistoryErrorType.Network
                            is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Error.Unauthorized ->
                                ListeningHistoryErrorType.Unauthorized
                            else -> ListeningHistoryErrorType.Unknown
                        }
                        Result.failure(ListeningHistoryException(errorType))
                    }
                }
            }
        }

    @Provides
    fun provideNotificationListScreenInteractor(
        notificationRepository: NotificationRepository,
    ): NotificationListScreenViewModel.Interactor =
        object : NotificationListScreenViewModel.Interactor {
            override fun getNotifications(): Flow<List<UiNotification>> =
                notificationRepository.notifications.map { notifications ->
                    notifications.map { it.toUiNotification() }
                }

            override suspend fun markAsRead(notificationId: String) {
                notificationRepository.markAsRead(notificationId)
            }

            override suspend fun markAllAsRead() {
                notificationRepository.markAllAsRead()
            }

            private fun com.lelloman.pezzottify.android.domain.notifications.Notification.toUiNotification(): UiNotification {
                return UiNotification(
                    id = id,
                    title = title,
                    body = body,
                    readAt = readAt,
                    createdAt = createdAt,
                    relativeTime = formatRelativeTime(createdAt),
                    albumId = getAlbumId(),
                )
            }

            private fun formatRelativeTime(timestamp: Long): String {
                val now = System.currentTimeMillis() / 1000
                val diff = now - timestamp

                return when {
                    diff < 60 -> "Just now"
                    diff < 3600 -> "${diff / 60}m ago"
                    diff < 86400 -> "${diff / 3600}h ago"
                    diff < 604800 -> "${diff / 86400}d ago"
                    else -> {
                        val date = java.text.SimpleDateFormat("MMM d", java.util.Locale.getDefault())
                            .format(java.util.Date(timestamp * 1000))
                        date
                    }
                }
            }
        }

    @Provides
    fun provideWhatsNewScreenInteractor(
        getWhatsNew: GetWhatsNew,
    ): WhatsNewScreenViewModel.Interactor =
        object : WhatsNewScreenViewModel.Interactor {
            override suspend fun getWhatsNew(limit: Int): Result<List<WhatsNewScreenViewModel.WhatsNewBatchData>> {
                return getWhatsNew(limit).map { response ->
                    response.batches.map { batch ->
                        WhatsNewScreenViewModel.WhatsNewBatchData(
                            batchId = batch.id,
                            batchName = batch.name,
                            description = batch.description,
                            closedAt = batch.closedAt,
                            artistsAdded = batch.summary.artists.added.size,
                            albumsAdded = batch.summary.albums.added.size,
                            tracksAdded = batch.summary.tracks.addedCount,
                            artistsUpdated = batch.summary.artists.updatedCount,
                            albumsUpdated = batch.summary.albums.updatedCount,
                            tracksUpdated = batch.summary.tracks.updatedCount,
                            albumIds = batch.summary.albums.added.map { it.id },
                        )
                    }
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
    DomainPermission.ServerAdmin -> UiPermission.ServerAdmin
    DomainPermission.ViewAnalytics -> UiPermission.ViewAnalytics
    DomainPermission.RequestContent -> UiPermission.RequestContent
    DomainPermission.DownloadManagerAdmin -> UiPermission.DownloadManagerAdmin
}