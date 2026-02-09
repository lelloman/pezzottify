package com.lelloman.pezzottify.android.ui

import android.content.Intent
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcAuthManager
import com.lelloman.pezzottify.android.domain.auth.usecase.HandleSessionExpired
import com.lelloman.pezzottify.android.domain.auth.usecase.IsLoggedIn
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogin
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformLogout
import com.lelloman.pezzottify.android.domain.auth.usecase.PerformOidcLogin
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.impression.RecordImpressionUseCase
import com.lelloman.pezzottify.android.domain.notifications.NotificationRepository
import com.lelloman.pezzottify.android.domain.notifications.getAlbumId
import com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer
import com.lelloman.pezzottify.android.domain.player.RepeatMode
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.remoteapi.response.ResolvedSearchResult
import com.lelloman.pezzottify.android.domain.remoteapi.response.SearchSection
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.settings.usecase.UpdateNotifyWhatsNewSetting
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.domain.statics.usecase.GetGenres
import com.lelloman.pezzottify.android.domain.statics.usecase.GetPopularContent
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWhatsNew
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformSearch
import com.lelloman.pezzottify.android.domain.statics.usecase.PerformStreamingSearch
import com.lelloman.pezzottify.android.domain.user.GetRecentlyViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.GetSearchHistoryEntriesUseCase
import com.lelloman.pezzottify.android.domain.user.LogSearchHistoryEntryUseCase
import com.lelloman.pezzottify.android.domain.user.LogViewedContentUseCase
import com.lelloman.pezzottify.android.domain.user.PermissionsStore
import com.lelloman.pezzottify.android.domain.user.SearchHistoryEntry
import com.lelloman.pezzottify.android.domain.user.ViewedContent
import com.lelloman.pezzottify.android.domain.usercontent.GetLikedStateUseCase
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSyncStatus
import com.lelloman.pezzottify.android.domain.usercontent.PlaylistSynchronizer
import com.lelloman.pezzottify.android.domain.usercontent.ToggleLikeUseCase
import com.lelloman.pezzottify.android.domain.usercontent.UserContentStore
import com.lelloman.pezzottify.android.domain.usercontent.UserPlaylistStore
import com.lelloman.pezzottify.android.domain.websocket.WebSocketManager
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.logging.LogFileManager
import com.lelloman.pezzottify.android.mapping.toAlbumAvailability
import com.lelloman.pezzottify.android.mapping.toAppFontFamily
import com.lelloman.pezzottify.android.mapping.toColorPalette
import com.lelloman.pezzottify.android.mapping.toConnectionState
import com.lelloman.pezzottify.android.mapping.toContentType
import com.lelloman.pezzottify.android.mapping.toPermission
import com.lelloman.pezzottify.android.mapping.toStorageInfo
import com.lelloman.pezzottify.android.mapping.toThemeMode
import com.lelloman.pezzottify.android.mapping.toTrackAvailability
import com.lelloman.pezzottify.android.oidc.OidcCallbackHandler
import com.lelloman.pezzottify.android.ui.screen.about.AboutScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.album.AlbumScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.artist.ArtistScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.track.TrackScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist.UiUserPlaylistDetails
import com.lelloman.pezzottify.android.ui.screen.main.content.userplaylist.UserPlaylistScreenViewModel
import com.lelloman.pezzottify.android.domain.playbacksession.PlaybackSessionHandler
import com.lelloman.pezzottify.android.ui.screen.main.devices.DevicesScreenState
import com.lelloman.pezzottify.android.ui.screen.main.devices.DevicesScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.devices.DeviceUiState
import com.lelloman.pezzottify.android.ui.screen.main.devices.DeviceSharePolicyUi
import com.lelloman.pezzottify.android.ui.screen.main.devices.DeviceSharePolicyUiState
import com.lelloman.pezzottify.android.mapping.toDeviceSharePolicyUi
import com.lelloman.pezzottify.android.ui.screen.main.genre.GenreListScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.genre.GenreScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenState
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.PopularAlbumState
import com.lelloman.pezzottify.android.ui.screen.main.home.PopularArtistState
import com.lelloman.pezzottify.android.ui.screen.main.home.PopularContentState
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import com.lelloman.pezzottify.android.ui.screen.main.library.LibraryScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.library.UiUserPlaylist
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryErrorType
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryException
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.ListeningHistoryScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.listeninghistory.UiListeningEvent
import com.lelloman.pezzottify.android.ui.screen.main.notifications.NotificationListScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.notifications.UiNotification
import com.lelloman.pezzottify.android.ui.screen.main.profile.ProfileScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.profile.stylesettings.StyleSettingsViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.GenreItem
import com.lelloman.pezzottify.android.ui.screen.main.search.PrimaryMatchType
import com.lelloman.pezzottify.android.ui.screen.main.search.SearchScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.search.StreamingAlbumSummary
import com.lelloman.pezzottify.android.ui.screen.main.search.StreamingArtistSummary
import com.lelloman.pezzottify.android.ui.screen.main.search.StreamingSearchResult
import com.lelloman.pezzottify.android.ui.screen.main.search.StreamingSearchSection
import com.lelloman.pezzottify.android.ui.screen.main.search.StreamingTrackSummary
import com.lelloman.pezzottify.android.ui.screen.main.settings.SettingsScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport.BugReportScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport.SubmitResult
import com.lelloman.pezzottify.android.ui.screen.main.settings.logviewer.LogViewerScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.main.whatsnew.WhatsNewScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.player.PlayerScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.player.RepeatModeUi
import com.lelloman.pezzottify.android.ui.screen.queue.QueueScreenViewModel
import com.lelloman.pezzottify.android.ui.screen.splash.SplashViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.components.ViewModelComponent
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import java.util.UUID
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylist as DomainPlaybackPlaylist
import com.lelloman.pezzottify.android.domain.player.PlaybackPlaylistContext as DomainPlaybackPlaylistContext
import com.lelloman.pezzottify.android.domain.statics.AlbumAvailability as DomainAlbumAvailability
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability as DomainTrackAvailability
import com.lelloman.pezzottify.android.domain.sync.Permission as DomainPermission
import com.lelloman.pezzottify.android.domain.usercontent.LikedContent as DomainLikedContent
import com.lelloman.pezzottify.android.domain.websocket.ConnectionState as DomainConnectionState
import com.lelloman.pezzottify.android.ui.component.ConnectionState as UiConnectionState
import com.lelloman.pezzottify.android.ui.model.LikedContent as UiLikedContent
import com.lelloman.pezzottify.android.ui.model.Permission as UiPermission
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylist as UiPlaybackPlaylist
import com.lelloman.pezzottify.android.ui.model.PlaybackPlaylistContext as UiPlaybackPlaylistContext
import com.lelloman.pezzottify.android.ui.model.StorageInfo as UiStorageInfo
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily as UiAppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette as UiColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode as UiThemeMode

private fun formatBatchDate(timestampSeconds: Long): String {
    val date = java.util.Date(timestampSeconds * 1000)
    val format = java.text.SimpleDateFormat("MMMM d, yyyy", java.util.Locale.getDefault())
    return format.format(date)
}

@InstallIn(ViewModelComponent::class)
@Module
class InteractorsModule {

    @Provides
    fun provideSplashInteractor(isLoggedIn: IsLoggedIn): SplashViewModel.Interactor =
        object : SplashViewModel.Interactor {
            override suspend fun isLoggedIn() = isLoggedIn()
        }

    @Provides
    fun provideSessionExpiredInteractor(
        handleSessionExpired: HandleSessionExpired,
    ): SessionExpiredViewModel.Interactor =
        object : SessionExpiredViewModel.Interactor {
            override fun sessionExpiredEvents(): Flow<Unit> = handleSessionExpired.events

            override suspend fun handleSessionExpired() {
                handleSessionExpired()
            }
        }

    @Provides
    fun provideLoginInteractor(
        performLogin: PerformLogin,
        performOidcLogin: PerformOidcLogin,
        oidcAuthManager: OidcAuthManager,
        oidcCallbackHandler: OidcCallbackHandler,
        deviceInfoProvider: DeviceInfoProvider,
        configStore: ConfigStore,
        authStore: AuthStore,
    ): LoginViewModel.Interactor = object : LoginViewModel.Interactor {
        override fun getInitialHost(): String = configStore.baseUrl.value

        override fun getInitialEmail(): String =
            authStore.getLastUsedHandle() ?: ""

        override val isLoggedIn: kotlinx.coroutines.flow.Flow<Boolean> =
            authStore.getAuthState().map { it is AuthState.LoggedIn }

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

        override fun oidcCallbacks(): SharedFlow<Intent> = oidcCallbackHandler.callbacks

        override suspend fun createOidcAuthIntent(): LoginViewModel.Interactor.OidcIntentResult {
            // Clear processed state when starting a new auth flow
            oidcCallbackHandler.clearProcessedState()
            val deviceInfo = deviceInfoProvider.getDeviceInfo()
            // Pass last used handle as login hint to pre-fill username on login page
            val loginHint = authStore.getLastUsedHandle()
            val intent = oidcAuthManager.createAuthorizationIntent(deviceInfo, loginHint)
            return if (intent != null) {
                LoginViewModel.Interactor.OidcIntentResult.Success(intent)
            } else {
                LoginViewModel.Interactor.OidcIntentResult.Error("OIDC not configured")
            }
        }

        override suspend fun handleOidcCallback(intent: Intent): LoginViewModel.Interactor.OidcLoginResult {
            // Prevent duplicate processing of the same callback
            if (oidcCallbackHandler.isAlreadyProcessed(intent)) {
                return LoginViewModel.Interactor.OidcLoginResult.Cancelled
            }
            oidcCallbackHandler.markAsProcessed(intent)

            val authResult = oidcAuthManager.handleAuthorizationResponse(intent)
            return when (val loginResult = performOidcLogin(authResult)) {
                is PerformOidcLogin.LoginResult.Success ->
                    LoginViewModel.Interactor.OidcLoginResult.Success

                is PerformOidcLogin.LoginResult.Cancelled ->
                    LoginViewModel.Interactor.OidcLoginResult.Cancelled

                is PerformOidcLogin.LoginResult.Error ->
                    LoginViewModel.Interactor.OidcLoginResult.Error(loginResult.message)
            }
        }
    }

    @Provides
    fun provideAboutScreenInteractor(
        buildInfo: BuildInfo,
        configStore: ConfigStore,
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
                permissions.map { it.toPermission() }.toSet()
            }
    }

    @Provides
    fun provideSettingsScreenInteractor(
        userSettingsStore: UserSettingsStore,
        storageMonitor: com.lelloman.pezzottify.android.domain.storage.StorageMonitor,
        updateNotifyWhatsNewSetting: UpdateNotifyWhatsNewSetting,
        logFileManager: LogFileManager,
        configStore: ConfigStore,
        permissionsStore: PermissionsStore,
        cacheManager: com.lelloman.pezzottify.android.domain.cache.CacheManager,
    ): SettingsScreenViewModel.Interactor = object : SettingsScreenViewModel.Interactor {
        override fun getThemeMode(): UiThemeMode = userSettingsStore.themeMode.value.toThemeMode()

        override fun getColorPalette(): UiColorPalette = userSettingsStore.colorPalette.value.toColorPalette()

        override fun getFontFamily(): UiAppFontFamily = userSettingsStore.fontFamily.value.toAppFontFamily()

        override fun isCacheEnabled(): Boolean = userSettingsStore.isInMemoryCacheEnabled.value

        override fun getStorageInfo(): UiStorageInfo = storageMonitor.storageInfo.value.toStorageInfo()

        override fun observeThemeMode(): Flow<UiThemeMode> =
            userSettingsStore.themeMode.map { it.toThemeMode() }

        override fun observeColorPalette(): Flow<UiColorPalette> =
            userSettingsStore.colorPalette.map { it.toColorPalette() }

        override fun observeFontFamily(): Flow<UiAppFontFamily> =
            userSettingsStore.fontFamily.map { it.toAppFontFamily() }

        override fun observeCacheEnabled() = userSettingsStore.isInMemoryCacheEnabled

        override fun observeStorageInfo(): Flow<UiStorageInfo> =
            storageMonitor.storageInfo.map { it.toStorageInfo() }

        override fun isNotifyWhatsNewEnabled(): Boolean =
            userSettingsStore.isNotifyWhatsNewEnabled.value

        override fun isSmartSearchEnabled(): Boolean = userSettingsStore.isSmartSearchEnabled.value

        override fun isExcludeUnavailableEnabled(): Boolean =
            userSettingsStore.isExcludeUnavailableEnabled.value

        override fun observeNotifyWhatsNewEnabled(): Flow<Boolean> =
            userSettingsStore.isNotifyWhatsNewEnabled

        override fun observeSmartSearchEnabled(): Flow<Boolean> =
            userSettingsStore.isSmartSearchEnabled

        override fun observeExcludeUnavailableEnabled(): Flow<Boolean> =
            userSettingsStore.isExcludeUnavailableEnabled

        override suspend fun setNotifyWhatsNewEnabled(enabled: Boolean) {
            updateNotifyWhatsNewSetting(enabled)
        }

        override fun setSmartSearchEnabled(enabled: Boolean) {
            userSettingsStore.setSmartSearchEnabled(enabled)
        }

        override fun setExcludeUnavailableEnabled(enabled: Boolean) {
            userSettingsStore.setExcludeUnavailableEnabled(enabled)
        }

        override suspend fun setThemeMode(themeMode: UiThemeMode) {
            userSettingsStore.setThemeMode(themeMode.toThemeMode())
        }

        override suspend fun setColorPalette(colorPalette: UiColorPalette) {
            userSettingsStore.setColorPalette(colorPalette.toColorPalette())
        }

        override suspend fun setFontFamily(fontFamily: UiAppFontFamily) {
            userSettingsStore.setFontFamily(fontFamily.toAppFontFamily())
        }

        override suspend fun setCacheEnabled(enabled: Boolean) {
            userSettingsStore.setInMemoryCacheEnabled(enabled)
        }

        override fun observeFileLoggingEnabled(): Flow<Boolean> =
            userSettingsStore.isFileLoggingEnabled

        override suspend fun setFileLoggingEnabled(enabled: Boolean) {
            userSettingsStore.setFileLoggingEnabled(enabled)
        }

        override fun isFileLoggingEnabled(): Boolean = userSettingsStore.isFileLoggingEnabled.value

        override fun hasLogFiles(): Boolean = logFileManager.hasLogs()

        override fun getLogFilesSize(): String = logFileManager.getFormattedLogSize()

        override fun getShareLogsIntent(): android.content.Intent =
            logFileManager.createShareIntent()

        override fun clearLogs() = logFileManager.clearLogs()

        override fun getBaseUrl(): String = configStore.baseUrl.value

        override suspend fun setBaseUrl(url: String): SettingsScreenViewModel.SetBaseUrlResult =
            when (configStore.setBaseUrl(url)) {
                ConfigStore.SetBaseUrlResult.Success -> SettingsScreenViewModel.SetBaseUrlResult.Success
                ConfigStore.SetBaseUrlResult.InvalidUrl -> SettingsScreenViewModel.SetBaseUrlResult.InvalidUrl
            }

        override fun observeCanReportBug(): Flow<Boolean> =
            permissionsStore.permissions.map { permissions ->
                permissions.contains(DomainPermission.ReportBug)
            }

        override suspend fun getCacheStats(): SettingsScreenViewModel.CacheStats {
            val stats = cacheManager.getStats()
            return SettingsScreenViewModel.CacheStats(
                staticsCacheSizeBytes = stats.totalStaticsCacheSizeBytes,
                imageCacheSizeBytes = stats.imageCacheSizeBytes,
            )
        }

        override suspend fun trimStaticsCache() {
            cacheManager.trimStaticsCache()
        }

        override suspend fun clearStaticsCache() {
            cacheManager.clearStaticsCache()
        }

        override suspend fun trimImageCache() {
            cacheManager.trimImageCache()
        }

        override suspend fun clearImageCache() {
            cacheManager.clearImageCache()
        }
    }

    @Provides
    fun provideStyleSettingsInteractor(
        userSettingsStore: UserSettingsStore,
    ): StyleSettingsViewModel.Interactor = object : StyleSettingsViewModel.Interactor {
        override fun getThemeMode(): UiThemeMode = userSettingsStore.themeMode.value.toThemeMode()

        override fun getColorPalette(): UiColorPalette = userSettingsStore.colorPalette.value.toColorPalette()

        override fun getFontFamily(): UiAppFontFamily = userSettingsStore.fontFamily.value.toAppFontFamily()

        override fun observeThemeMode(): Flow<UiThemeMode> =
            userSettingsStore.themeMode.map { it.toThemeMode() }

        override fun observeColorPalette(): Flow<UiColorPalette> =
            userSettingsStore.colorPalette.map { it.toColorPalette() }

        override fun observeFontFamily(): Flow<UiAppFontFamily> =
            userSettingsStore.fontFamily.map { it.toAppFontFamily() }

        override suspend fun setThemeMode(themeMode: UiThemeMode) {
            userSettingsStore.setThemeMode(themeMode.toThemeMode())
        }

        override suspend fun setColorPalette(colorPalette: UiColorPalette) {
            userSettingsStore.setColorPalette(colorPalette.toColorPalette())
        }

        override suspend fun setFontFamily(fontFamily: UiAppFontFamily) {
            userSettingsStore.setFontFamily(fontFamily.toAppFontFamily())
        }
    }

    @Provides
    fun provideLogViewerScreenInteractor(
        logFileManager: LogFileManager,
    ): LogViewerScreenViewModel.Interactor = object : LogViewerScreenViewModel.Interactor {
        override fun getLogContent(): String = logFileManager.getLogContent()
    }

    @Provides
    fun provideBugReportScreenInteractor(
        logFileManager: LogFileManager,
        remoteApiClient: RemoteApiClient,
        buildInfo: BuildInfo,
        deviceInfoProvider: DeviceInfoProvider,
    ): BugReportScreenViewModel.Interactor = object : BugReportScreenViewModel.Interactor {
        private val maxLogSize = 1024 * 1024 // 1MB

        override fun getLogs(): String? = logFileManager.getLogContent()
            .takeIf { it.isNotBlank() }
            ?.let { content ->
                if (content.length > maxLogSize) {
                    val truncated = content.substring(content.length - maxLogSize)
                    // Skip partial first line from the cut point
                    val firstNewline = truncated.indexOf('\n')
                    if (firstNewline >= 0) truncated.substring(firstNewline + 1) else truncated
                } else {
                    content
                }
            }

        override suspend fun submitBugReport(
            title: String?,
            description: String,
            logs: String?,
        ): SubmitResult {
            val info = deviceInfoProvider.getDeviceInfo()
            val deviceInfo = "${info.deviceName ?: info.deviceType} (${info.osInfo ?: "Unknown"})"
            return when (val result = remoteApiClient.submitBugReport(
                title = title,
                description = description,
                clientVersion = buildInfo.versionName,
                deviceInfo = deviceInfo,
                logs = logs,
                attachments = null,
            )) {
                is RemoteApiResponse.Success -> SubmitResult.Success
                is RemoteApiResponse.Error -> SubmitResult.Error(result.toString())
            }
        }
    }

    @Provides
    fun provideSearchScreenInteractor(
        performSearch: PerformSearch,
        performStreamingSearch: PerformStreamingSearch,
        loggerFactory: LoggerFactory,
        getRecentlyViewedContent: GetRecentlyViewedContentUseCase,
        getSearchHistoryEntries: GetSearchHistoryEntriesUseCase,
        logSearchHistoryEntry: LogSearchHistoryEntryUseCase,
        getWhatsNew: GetWhatsNew,
        getGenres: GetGenres,
        userSettingsStore: UserSettingsStore,
        configStore: ConfigStore,
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

            override fun streamingSearch(query: String): Flow<StreamingSearchSection> {
                val excludeUnavailable = userSettingsStore.isExcludeUnavailableEnabled.value
                logger.debug("streamingSearch($query, excludeUnavailable=$excludeUnavailable)")
                val baseUrl = configStore.baseUrl.value
                return performStreamingSearch(query, excludeUnavailable).map { section ->
                    mapToUiSection(section, baseUrl)
                }.filterNotNull()
            }

            private fun mapToUiSection(
                section: SearchSection,
                baseUrl: String
            ): StreamingSearchSection? {
                return when (section) {
                    is SearchSection.PrimaryArtist -> {
                        val artist = section.item as? ResolvedSearchResult.Artist ?: return null
                        StreamingSearchSection.PrimaryMatch(
                            id = artist.id,
                            name = artist.name,
                            type = PrimaryMatchType.Artist,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, artist.imageId),
                            confidence = section.confidence,
                        )
                    }

                    is SearchSection.PrimaryAlbum -> {
                        val album = section.item as? ResolvedSearchResult.Album ?: return null
                        StreamingSearchSection.PrimaryMatch(
                            id = album.id,
                            name = album.name,
                            type = PrimaryMatchType.Album,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, album.imageId),
                            confidence = section.confidence,
                            artistNames = album.artistsIdsNames.map { it[1] },
                            year = album.year?.toInt(),
                        )
                    }

                    is SearchSection.PrimaryTrack -> {
                        val track = section.item as? ResolvedSearchResult.Track ?: return null
                        StreamingSearchSection.PrimaryMatch(
                            id = track.id,
                            name = track.name,
                            type = PrimaryMatchType.Track,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, track.imageId),
                            confidence = section.confidence,
                            artistNames = track.artistsIdsNames.map { it[1] },
                            durationMs = track.duration.toLong() * 1000,
                        )
                    }

                    is SearchSection.PopularBy -> {
                        StreamingSearchSection.PopularTracks(
                            targetId = section.targetId,
                            tracks = section.items.map { track ->
                                StreamingTrackSummary(
                                    id = track.id,
                                    name = track.name,
                                    durationMs = track.durationMs,
                                    trackNumber = track.trackNumber,
                                    albumId = track.albumId,
                                    albumName = track.albumName,
                                    artistNames = track.artistNames,
                                    imageUrl = ImageUrlProvider.buildImageUrl(
                                        baseUrl,
                                        track.imageId
                                    ),
                                )
                            }
                        )
                    }

                    is SearchSection.AlbumsBy -> {
                        StreamingSearchSection.ArtistAlbums(
                            targetId = section.targetId,
                            albums = section.items.map { album ->
                                StreamingAlbumSummary(
                                    id = album.id,
                                    name = album.name,
                                    releaseYear = album.releaseYear,
                                    trackCount = album.trackCount,
                                    imageUrl = ImageUrlProvider.buildImageUrl(
                                        baseUrl,
                                        album.imageId
                                    ),
                                    artistNames = album.artistNames,
                                    availability = DomainAlbumAvailability.fromServerString(album.availability)
                                        .toAlbumAvailability(),
                                )
                            }
                        )
                    }

                    is SearchSection.TracksFrom -> {
                        StreamingSearchSection.AlbumTracks(
                            targetId = section.targetId,
                            tracks = section.items.map { track ->
                                StreamingTrackSummary(
                                    id = track.id,
                                    name = track.name,
                                    durationMs = track.durationMs,
                                    trackNumber = track.trackNumber,
                                    albumId = track.albumId,
                                    albumName = track.albumName,
                                    artistNames = track.artistNames,
                                    imageUrl = ImageUrlProvider.buildImageUrl(
                                        baseUrl,
                                        track.imageId
                                    ),
                                )
                            }
                        )
                    }

                    is SearchSection.RelatedArtists -> {
                        StreamingSearchSection.RelatedArtists(
                            targetId = section.targetId,
                            artists = section.items.map { artist ->
                                StreamingArtistSummary(
                                    id = artist.id,
                                    name = artist.name,
                                    imageUrl = ImageUrlProvider.buildImageUrl(
                                        baseUrl,
                                        artist.imageId
                                    ),
                                )
                            }
                        )
                    }

                    is SearchSection.MoreResults -> {
                        StreamingSearchSection.MoreResults(
                            results = section.items.map { mapSearchResult(it, baseUrl) }
                        )
                    }

                    is SearchSection.Results -> {
                        StreamingSearchSection.AllResults(
                            results = section.items.map { mapSearchResult(it, baseUrl) }
                        )
                    }

                    is SearchSection.Done -> {
                        StreamingSearchSection.Done(totalTimeMs = section.totalTimeMs)
                    }
                }
            }

            private fun mapSearchResult(
                result: ResolvedSearchResult,
                baseUrl: String
            ): StreamingSearchResult {
                return when (result) {
                    is ResolvedSearchResult.Artist -> {
                        StreamingSearchResult.Artist(
                            id = result.id,
                            name = result.name,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, result.imageId),
                        )
                    }

                    is ResolvedSearchResult.Album -> {
                        StreamingSearchResult.Album(
                            id = result.id,
                            name = result.name,
                            artistNames = result.artistsIdsNames.map { it[1] },
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, result.imageId),
                            year = result.year?.toInt(),
                            availability = DomainAlbumAvailability.fromServerString(result.availability)
                                .toAlbumAvailability(),
                        )
                    }

                    is ResolvedSearchResult.Track -> {
                        StreamingSearchResult.Track(
                            id = result.id,
                            name = result.name,
                            artistNames = result.artistsIdsNames.map { it[1] },
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, result.imageId),
                            albumId = result.albumId,
                            durationMs = result.duration.toLong() * 1000,
                            availability = DomainTrackAvailability.fromServerString(result.availability)
                                .toTrackAvailability(),
                        )
                    }
                }
            }

            override fun isStreamingSearchEnabled(): Boolean =
                userSettingsStore.isSmartSearchEnabled.value

            override fun observeStreamingSearchEnabled(): Flow<Boolean> =
                userSettingsStore.isSmartSearchEnabled

            override fun setStreamingSearchEnabled(enabled: Boolean) {
                userSettingsStore.setSmartSearchEnabled(enabled)
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

            override suspend fun getWhatsNew(limit: Int): Result<List<SearchScreenViewModel.WhatsNewBatchData>> {
                logger.debug("getWhatsNew(limit=$limit)")
                return getWhatsNew(limit).map { response ->
                    response.batches.map { batch ->
                        SearchScreenViewModel.WhatsNewBatchData(
                            batchId = batch.id,
                            batchName = batch.name ?: formatBatchDate(batch.closedAt),
                            closedAt = batch.closedAt,
                            addedAlbumIds = batch.summary.albums.added.map { it.id },
                        )
                    }
                }
            }

            override suspend fun getGenres(limit: Int): Result<List<GenreItem>> {
                logger.debug("getGenres(limit=$limit)")
                return getGenres(limit).map { response ->
                    response.map { genre ->
                        GenreItem(
                            name = genre.name,
                            trackCount = genre.trackCount,
                        )
                    }
                }
            }
        }

    @Provides
    fun provideAlbumScreenInteractor(
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        recordImpressionUseCase: RecordImpressionUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        userPlaylistStore: UserPlaylistStore,
        staticsStore: StaticsStore,
        playlistSynchronizer: PlaylistSynchronizer,
        permissionsStore: PermissionsStore,
        downloadStatusRepository: com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository,
        requestAlbumDownloadUseCase: com.lelloman.pezzottify.android.domain.download.RequestAlbumDownloadUseCase,
        getMyDownloadRequestsUseCase: com.lelloman.pezzottify.android.domain.download.GetMyDownloadRequestsUseCase,
    ): AlbumScreenViewModel.Interactor = object : AlbumScreenViewModel.Interactor {
        override fun playAlbum(albumId: String) {
            player.loadAlbum(albumId)
        }

        override fun playTrack(albumId: String, trackId: String) {
            player.loadAlbum(albumId, trackId)
        }

        override fun logViewedAlbum(albumId: String) {
            logViewedContentUseCase(albumId, ViewedContent.Type.Album)
            recordImpressionUseCase.recordAlbum(albumId)
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
            userPlaylistStore.createOrUpdatePlaylist(
                id,
                name,
                emptyList(),
                PlaylistSyncStatus.PendingCreate
            )
            playlistSynchronizer.wakeUp()
        }

        override fun hasRequestContentPermission(): Flow<Boolean> =
            permissionsStore.permissions.map { permissions ->
                permissions.contains(DomainPermission.RequestContent)
            }

        override fun observeDownloadRequestStatus(albumId: String): Flow<AlbumScreenViewModel.DownloadRequestStatus?> {
            // Flow that emits initial state from API, then continues with WebSocket updates
            return kotlinx.coroutines.flow.flow {
                // First, check the initial state via API
                val initialResult = getMyDownloadRequestsUseCase(limit = 100, offset = 0)
                val initialRequest =
                    initialResult.getOrNull()?.requests?.find { it.contentId == albumId }

                var currentStatus = initialRequest?.let { request ->
                    AlbumScreenViewModel.DownloadRequestStatus(
                        status = when (request.status) {
                            com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Pending ->
                                com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Pending

                            com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.InProgress ->
                                com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.InProgress

                            com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.RetryWaiting ->
                                com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Pending

                            com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Completed ->
                                com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Completed

                            com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Failed ->
                                com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Failed
                        },
                        queuePosition = request.queuePosition,
                        progress = request.progress?.let { progress ->
                            com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestProgress(
                                completed = progress.completed,
                                total = progress.totalChildren,
                            )
                        },
                    )
                }
                emit(currentStatus)

                // Then collect WebSocket updates - only update when we get actual status info
                downloadStatusRepository.observeStatus(albumId).collect { statusInfo ->
                    if (statusInfo != null) {
                        currentStatus = AlbumScreenViewModel.DownloadRequestStatus(
                            status = when (statusInfo.status) {
                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Pending ->
                                    com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Pending

                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.InProgress ->
                                    com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.InProgress

                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.RetryWaiting ->
                                    com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Pending

                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Completed ->
                                    com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Completed

                                com.lelloman.pezzottify.android.domain.remoteapi.response.DownloadQueueStatus.Failed ->
                                    com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestStatus.Failed
                            },
                            queuePosition = statusInfo.queuePosition,
                            progress = statusInfo.progress?.let { progress ->
                                com.lelloman.pezzottify.android.ui.screen.main.content.album.RequestProgress(
                                    completed = progress.completed,
                                    total = progress.totalChildren,
                                )
                            },
                        )
                        emit(currentStatus)
                    }
                    // If statusInfo is null, don't emit - keep the current status from initial check
                }
            }
        }

        override suspend fun requestAlbumDownload(
            albumId: String,
            albumName: String,
            artistName: String
        ): Result<Unit> {
            val result = requestAlbumDownloadUseCase(albumId, albumName, artistName)
            // On success, immediately update the status repository so the UI reflects the new state
            // without waiting for the WebSocket event
            result.onSuccess { response ->
                downloadStatusRepository.onRequestCreated(
                    requestId = response.requestId,
                    contentId = albumId,
                    contentName = albumName,
                    artistName = artistName,
                    queuePosition = 0, // Position not provided in response, will be updated via WebSocket
                )
            }
            return result.map { }
        }
    }

    @Provides
    fun provideArtistScreenInteractor(
        logViewedContentUseCase: LogViewedContentUseCase,
        recordImpressionUseCase: RecordImpressionUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        staticsProvider: StaticsProvider,
    ): ArtistScreenViewModel.Interactor = object : ArtistScreenViewModel.Interactor {
        override fun logViewedArtist(artistId: String) {
            logViewedContentUseCase(artistId, ViewedContent.Type.Artist)
            recordImpressionUseCase.recordArtist(artistId)
        }

        override fun isLiked(contentId: String): Flow<Boolean> =
            getLikedStateUseCase(contentId)

        override fun toggleLike(contentId: String, currentlyLiked: Boolean) {
            toggleLikeUseCase(contentId, DomainLikedContent.ContentType.Artist, currentlyLiked)
        }

        override fun observeDiscographyState(artistId: String) =
            staticsProvider.observeDiscographyState(artistId).map { domainState ->
                ArtistScreenViewModel.DiscographyUiState(
                    albumIds = domainState.albumIds,
                    hasMore = domainState.hasMore,
                    isLoading = domainState.isLoading
                )
            }

        override suspend fun fetchFirstDiscographyPage(artistId: String) {
            staticsProvider.fetchFirstDiscographyPage(artistId)
        }

        override suspend fun fetchMoreDiscography(artistId: String) {
            staticsProvider.fetchMoreDiscography(artistId)
        }

        override suspend fun retryErroredItems(itemIds: List<String>) {
            staticsProvider.retryErroredItems(itemIds)
        }
    }

    @Provides
    fun provideTrackScreenInteractor(
        player: PezzottifyPlayer,
        logViewedContentUseCase: LogViewedContentUseCase,
        recordImpressionUseCase: RecordImpressionUseCase,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
    ): TrackScreenViewModel.Interactor = object : TrackScreenViewModel.Interactor {
        override fun playTrack(albumId: String, trackId: String) {
            // Always play the track by loading the album starting from this track
            player.loadAlbum(albumId, trackId)
        }

        override fun logViewedTrack(trackId: String) {
            logViewedContentUseCase(trackId, ViewedContent.Type.Track)
            recordImpressionUseCase.recordTrack(trackId)
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
        playbackModeManager: com.lelloman.pezzottify.android.domain.player.PlaybackModeManager,
        playbackSessionHandler: PlaybackSessionHandler,
    ): MainScreenViewModel.Interactor =
        object : MainScreenViewModel.Interactor {

            val logger = loggerFactory.getLogger(MainScreenViewModel.Interactor::class)

            override fun getNotificationUnreadCount(): Flow<Int> =
                notificationRepository.unreadCount

            override fun getRemoteDeviceName(): Flow<String?> =
                playbackModeManager.mode.map { mode ->
                    (mode as? com.lelloman.pezzottify.android.domain.player.PlaybackMode.Remote)?.deviceName
                }

            override fun getHasOtherDeviceConnected(): Flow<Boolean> =
                combine(
                    playbackSessionHandler.connectedDevices,
                    playbackSessionHandler.myDeviceId,
                ) { devices, myDeviceId ->
                    if (myDeviceId != null) {
                        devices.any { it.id != myDeviceId }
                    } else {
                        devices.size > 1
                    }
                }

            override fun getPlaybackState(): Flow<MainScreenViewModel.Interactor.PlaybackState?> =
                playbackMetadataProvider.queueState
                    .combine(player.isPlaying) { queueState, isPlaying -> queueState to isPlaying }
                    .combine(player.currentTrackPercent) { (queueState, isPlaying), trackPercent ->
                        val currentTrack = queueState?.currentTrack
                        when {
                            currentTrack != null -> {
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
                            }
                            // Show loading state when metadata is being loaded
                            queueState?.isLoading == true -> {
                                MainScreenViewModel.Interactor.PlaybackState.Loading
                            }
                            else -> null
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
            webSocketManager.connectionState.map { it.toConnectionState() }
                .stateIn(
                    scope,
                    SharingStarted.Eagerly,
                    webSocketManager.connectionState.value.toConnectionState()
                )

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
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, album.id),
                            artistNames = album.artistNames,
                        )
                    },
                    artists = popularContent.artists.map { artist ->
                        PopularArtistState(
                            id = artist.id,
                            name = artist.name,
                            imageUrl = ImageUrlProvider.buildImageUrl(baseUrl, artist.id),
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
        playbackModeManager: com.lelloman.pezzottify.android.domain.player.PlaybackModeManager,
        playbackSessionHandler: PlaybackSessionHandler,
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
                        PlaybackData2(
                            data.queueState,
                            data.isPlaying,
                            data.trackPercent,
                            data.progressSec,
                            volumeState.volume,
                            volumeState.isMuted
                        )
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
                        PlaybackData3(
                            data.queueState,
                            data.isPlaying,
                            data.trackPercent,
                            data.progressSec,
                            data.volume,
                            data.isMuted,
                            shuffleEnabled
                        )
                    }
                    .combine(player.repeatMode) { data, repeatMode ->
                        data class PlaybackData4(
                            val queueState: com.lelloman.pezzottify.android.domain.player.PlaybackQueueState?,
                            val isPlaying: Boolean,
                            val trackPercent: Float?,
                            val progressSec: Int?,
                            val volume: Float,
                            val isMuted: Boolean,
                            val shuffleEnabled: Boolean,
                            val repeatMode: RepeatMode,
                        )
                        PlaybackData4(
                            data.queueState,
                            data.isPlaying,
                            data.trackPercent,
                            data.progressSec,
                            data.volume,
                            data.isMuted,
                            data.shuffleEnabled,
                            repeatMode
                        )
                    }
                    .combine(player.playerError) { data, playerError ->
                        val currentTrack = data.queueState?.currentTrack
                        if (currentTrack != null || playerError != null) {
                            val currentIndex = data.queueState?.currentIndex ?: 0
                            val tracksCount = data.queueState?.tracks?.size ?: 0
                            val isRemote = playbackModeManager.mode.value is com.lelloman.pezzottify.android.domain.player.PlaybackMode.Remote
                            val hasNext = isRemote || currentIndex < tracksCount - 1
                            val hasPrevious = isRemote || currentIndex > 0
                            val repeatModeUi = when (data.repeatMode) {
                                RepeatMode.OFF -> RepeatModeUi.OFF
                                RepeatMode.ALL -> RepeatModeUi.ALL
                                RepeatMode.ONE -> RepeatModeUi.ONE
                            }
                            PlayerScreenViewModel.Interactor.PlaybackState.Loaded(
                                isPlaying = data.isPlaying,
                                trackId = currentTrack?.trackId ?: "",
                                trackName = currentTrack?.trackName ?: "",
                                albumId = currentTrack?.albumId ?: "",
                                albumName = currentTrack?.albumName ?: "",
                                albumImageUrl = currentTrack?.artworkUrl,
                                artists = currentTrack?.artistNames?.mapIndexed { index, name ->
                                    com.lelloman.pezzottify.android.ui.content.ArtistInfo(
                                        id = index.toString(),
                                        name = name,
                                    )
                                } ?: emptyList(),
                                trackPercent = data.trackPercent ?: 0f,
                                trackProgressSec = data.progressSec ?: 0,
                                trackDurationSec = currentTrack?.durationSeconds ?: 0,
                                hasNextTrack = hasNext,
                                hasPreviousTrack = hasPrevious,
                                volume = data.volume,
                                isMuted = data.isMuted,
                                shuffleEnabled = data.shuffleEnabled,
                                repeatMode = repeatModeUi,
                                playerError = playerError?.let {
                                    com.lelloman.pezzottify.android.ui.screen.player.PlayerErrorUi(
                                        message = it.message,
                                        isRecoverable = it.isRecoverable
                                    )
                                },
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

            override fun retry() = player.retry()

            override fun getRemoteDeviceName(): Flow<String?> =
                playbackModeManager.mode.map { mode ->
                    (mode as? com.lelloman.pezzottify.android.domain.player.PlaybackMode.Remote)?.deviceName
                }

            override fun getHasOtherDeviceConnected(): Flow<Boolean> =
                combine(
                    playbackSessionHandler.connectedDevices,
                    playbackSessionHandler.myDeviceId,
                ) { devices, myDeviceId ->
                    if (myDeviceId != null) {
                        devices.any { it.id != myDeviceId }
                    } else {
                        devices.size > 1
                    }
                }

            override fun exitRemoteMode() {
                playbackModeManager.exitRemoteMode()
            }
        }

    @Provides
    fun provideQueueScreenInteractor(
        player: PezzottifyPlayer,
        playbackMetadataProvider: com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider,
        playbackModeManager: com.lelloman.pezzottify.android.domain.player.PlaybackModeManager,
        userPlaylistStore: UserPlaylistStore,
        getLikedStateUseCase: GetLikedStateUseCase,
        toggleLikeUseCase: ToggleLikeUseCase,
        playlistSynchronizer: PlaylistSynchronizer,
    ): QueueScreenViewModel.Interactor =
        object : QueueScreenViewModel.Interactor {
            override fun getIsRemote(): Flow<Boolean> =
                playbackModeManager.mode.map { mode ->
                    mode is com.lelloman.pezzottify.android.domain.player.PlaybackMode.Remote
                }

            override fun getQueueState(): Flow<QueueScreenViewModel.Interactor.QueueState?> =
                playbackMetadataProvider.queueState
                    .combine(player.playbackPlaylist) { queueState, playlist ->
                        queueState to playlist
                    }
                    .combine(playbackModeManager.mode) { (queueState, playlist), mode ->
                        if (queueState != null && (playlist != null || mode is com.lelloman.pezzottify.android.domain.player.PlaybackMode.Remote)) {
                            val (contextType, contextName, canSave) = if (playlist != null) {
                                val playlistContext = playlist.context
                                when (playlistContext) {
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
                            } else {
                                Triple(
                                    com.lelloman.pezzottify.android.ui.screen.queue.QueueContextType.Unknown,
                                    "",
                                    false,
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
                                        availability = track.availability.toTrackAvailability(),
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

            override fun removeTrack(index: Int) = player.removeTrackAtIndex(index)

            override fun playTrackDirectly(trackId: String) = player.loadSingleTrack(trackId)

            override fun addTrackToQueue(trackId: String) =
                player.addTracksToPlaylist(listOf(trackId))

            override suspend fun addTrackToPlaylist(trackId: String, playlistId: String) {
                userPlaylistStore.addTrackToPlaylist(playlistId, trackId)
                playlistSynchronizer.wakeUp()
            }

            override suspend fun createPlaylist(name: String) {
                val id = UUID.randomUUID().toString()
                userPlaylistStore.createOrUpdatePlaylist(
                    id,
                    name,
                    emptyList(),
                    PlaylistSyncStatus.PendingCreate
                )
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
                userPlaylistStore.createOrUpdatePlaylist(
                    id,
                    name,
                    emptyList(),
                    PlaylistSyncStatus.PendingCreate
                )
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
                userPlaylistStore.createOrUpdatePlaylist(
                    id,
                    name,
                    emptyList(),
                    PlaylistSyncStatus.PendingCreate
                )
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
        downloadStatusRepository: com.lelloman.pezzottify.android.domain.download.DownloadStatusRepository,
    ): com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.Interactor =
        object :
            com.lelloman.pezzottify.android.ui.screen.main.myrequests.MyRequestsScreenViewModel.Interactor {
            override suspend fun getMyRequests(
                limit: Int,
                offset: Int
            ): Result<List<com.lelloman.pezzottify.android.ui.screen.main.myrequests.UiDownloadRequest>> {
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
    fun provideGenreListScreenInteractor(
        getGenres: GetGenres,
    ): GenreListScreenViewModel.Interactor =
        object : GenreListScreenViewModel.Interactor {
            override suspend fun getGenres(limit: Int): Result<List<GenreListScreenViewModel.GenreData>> {
                return getGenres(limit).map { genres ->
                    genres.map { genre ->
                        GenreListScreenViewModel.GenreData(
                            name = genre.name,
                            trackCount = genre.trackCount,
                        )
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
                        val date =
                            java.text.SimpleDateFormat("MMM d", java.util.Locale.getDefault())
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
                        // Derive batch name from closedAt timestamp if not provided
                        val batchName = batch.name ?: formatBatchDate(batch.closedAt)
                        WhatsNewScreenViewModel.WhatsNewBatchData(
                            batchId = batch.id,
                            batchName = batchName,
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

    @Provides
    fun provideGenreScreenInteractor(
        getGenreTracks: com.lelloman.pezzottify.android.domain.statics.usecase.GetGenreTracks,
        player: com.lelloman.pezzottify.android.domain.player.PezzottifyPlayer,
    ): GenreScreenViewModel.Interactor = object : GenreScreenViewModel.Interactor {
        override suspend fun getGenreTracks(genreName: String): Result<GenreScreenViewModel.GenreTracksData> {
            return getGenreTracks(genreName).map { response ->
                GenreScreenViewModel.GenreTracksData(
                    trackIds = response.trackIds,
                    total = response.total,
                )
            }
        }

        override fun loadSingleTrack(trackId: String) {
            player.loadSingleTrack(trackId)
        }

        override fun addTracksToPlaylist(tracksIds: List<String>) {
            player.addTracksToPlaylist(tracksIds)
        }
    }

    @Provides
    fun provideDevicesScreenInteractor(
        playbackSessionHandler: PlaybackSessionHandler,
        playbackMetadataProvider: com.lelloman.pezzottify.android.domain.player.PlaybackMetadataProvider,
        player: PezzottifyPlayer,
        configStore: ConfigStore,
        playbackModeManager: com.lelloman.pezzottify.android.domain.player.PlaybackModeManager,
        remoteApiClient: RemoteApiClient,
        deviceInfoProvider: com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider,
    ): DevicesScreenViewModel.Interactor = object : DevicesScreenViewModel.Interactor {
        override fun observeDevicesScreenState(): Flow<DevicesScreenState> =
            combine(
                playbackSessionHandler.connectedDevices,
                playbackSessionHandler.myDeviceId,
                playbackSessionHandler.otherDeviceStates,
                playbackMetadataProvider.queueState,
                player.isPlaying,
                player.currentTrackProgressSec,
            ) { values ->
                @Suppress("UNCHECKED_CAST")
                val connectedDevices = values[0] as List<com.lelloman.pezzottify.android.domain.playbacksession.ConnectedDevice>
                val myDeviceId = values[1] as Int?
                @Suppress("UNCHECKED_CAST")
                val otherStates = values[2] as Map<Int, com.lelloman.pezzottify.android.domain.playbacksession.RemotePlaybackState>
                val queueState = values[3] as com.lelloman.pezzottify.android.domain.player.PlaybackQueueState?
                val isPlaying = values[4] as Boolean
                val progressSec = values[5] as Int?

                val baseUrl = configStore.baseUrl.value.trimEnd('/')

                val devices = connectedDevices.map { device ->
                    val isThisDevice = device.id == myDeviceId
                    if (isThisDevice) {
                        val currentTrack = queueState?.currentTrack
                        DeviceUiState(
                            id = device.id,
                            name = device.name,
                            deviceType = device.deviceType,
                            isThisDevice = true,
                            trackTitle = currentTrack?.trackName,
                            artistName = currentTrack?.artistNames?.firstOrNull(),
                            albumImageUrl = currentTrack?.artworkUrl,
                            isPlaying = isPlaying,
                            positionSec = (progressSec ?: 0).toDouble(),
                            durationMs = (currentTrack?.durationSeconds?.toLong() ?: 0L) * 1000L,
                        )
                    } else {
                        val remoteState = otherStates[device.id]
                        DeviceUiState(
                            id = device.id,
                            name = device.name,
                            deviceType = device.deviceType,
                            isThisDevice = false,
                            trackTitle = remoteState?.currentTrack?.title,
                            artistName = remoteState?.currentTrack?.artistName,
                            albumImageUrl = remoteState?.currentTrack?.imageId?.let { "$baseUrl/v1/content/image/$it" },
                            isPlaying = remoteState?.isPlaying ?: false,
                            positionSec = remoteState?.position ?: 0.0,
                            durationMs = remoteState?.currentTrack?.durationMs ?: 0L,
                            timestamp = remoteState?.receivedAt ?: 0L,
                        )
                    }
                }

                DevicesScreenState(
                    devices = devices,
                    thisDeviceId = myDeviceId,
                )
            }

        override fun observeRemoteControlDeviceId(): Flow<Int?> =
            playbackModeManager.mode.map { mode ->
                (mode as? com.lelloman.pezzottify.android.domain.player.PlaybackMode.Remote)?.deviceId
            }

        override fun sendCommand(command: String, payload: Map<String, Any?>, targetDeviceId: Int) {
            playbackSessionHandler.sendCommand(command, payload, targetDeviceId)
        }

        override fun enterRemoteMode(deviceId: Int, deviceName: String) {
            playbackModeManager.enterRemoteMode(deviceId, deviceName)
            playbackSessionHandler.requestQueue(deviceId)
        }

        override fun exitRemoteMode() {
            playbackModeManager.exitRemoteMode()
        }

        override suspend fun fetchDeviceSharePolicy(deviceId: Int): DevicesScreenViewModel.DeviceSharePolicyResult {
            return when (val response = remoteApiClient.getDevices()) {
                is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Success -> {
                    val device = response.data.devices.firstOrNull { it.id == deviceId }
                    if (device != null) {
                        DevicesScreenViewModel.DeviceSharePolicyResult.Success(
                            device.sharePolicy.toDeviceSharePolicyUi()
                        )
                    } else {
                        DevicesScreenViewModel.DeviceSharePolicyResult.Error("Device not found")
                    }
                }
                is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Error -> {
                    DevicesScreenViewModel.DeviceSharePolicyResult.Error("Failed to load policy")
                }
            }
        }

        override suspend fun resolveLocalDeviceId(): Int? {
            val deviceUuid = deviceInfoProvider.getDeviceInfo().deviceUuid
            return when (val response = remoteApiClient.getDevices()) {
                is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Success -> {
                    response.data.devices.firstOrNull { it.deviceUuid == deviceUuid }?.id
                }
                is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Error -> null
            }
        }

        override suspend fun updateDeviceSharePolicy(
            deviceId: Int,
            state: DeviceSharePolicyUiState
        ): DevicesScreenViewModel.DeviceSharePolicyResult {
            fun parseIds(input: String): List<Int> = input
                .split(",")
                .map { it.trim() }
                .filter { it.isNotEmpty() }
                .mapNotNull { it.toIntOrNull() }

            val allowRoles = mutableListOf<String>()
            if (state.allowAdmin) allowRoles.add("admin")
            if (state.allowRegular) allowRoles.add("regular")

            val request = com.lelloman.pezzottify.android.domain.remoteapi.request.DeviceSharePolicyRequest(
                mode = state.mode,
                allowUsers = if (state.mode == "custom") parseIds(state.allowUsers) else emptyList(),
                allowRoles = if (state.mode == "custom") allowRoles else emptyList(),
                denyUsers = if (state.mode == "custom") parseIds(state.denyUsers) else emptyList(),
            )

            return when (val response = remoteApiClient.updateDeviceSharePolicy(deviceId, request)) {
                is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Success -> {
                    DevicesScreenViewModel.DeviceSharePolicyResult.Success(
                        response.data.toDeviceSharePolicyUi()
                    )
                }
                is com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse.Error -> {
                    DevicesScreenViewModel.DeviceSharePolicyResult.Error("Failed to save policy")
                }
            }
        }
    }

    // Manual mapping for LikedContent (domain is interface with extra fields, UI is data class)

    private fun DomainLikedContent.toUi(): UiLikedContent = UiLikedContent(
        contentId = contentId,
        contentType = contentType.toContentType(),
        isLiked = isLiked
    )

    private fun List<DomainLikedContent>.toUi(): List<UiLikedContent> = map { it.toUi() }
}
