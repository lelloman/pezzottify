package com.lelloman.pezzottify.android.ui.screen.main.settings

import android.content.Intent
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.model.StorageInfo
import com.lelloman.pezzottify.android.ui.model.StoragePressureLevel
import com.lelloman.pezzottify.android.ui.theme.AppFontFamily
import com.lelloman.pezzottify.android.ui.theme.ColorPalette
import com.lelloman.pezzottify.android.ui.theme.ThemeMode
import io.mockk.mockk
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SettingsScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()

    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var viewModel: SettingsScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun createViewModel() {
        viewModel = SettingsScreenViewModel(fakeInteractor)
    }

    @Test
    fun `initial state loads settings from interactor`() = runTest {
        fakeInteractor.configureThemeMode(ThemeMode.Dark)
        fakeInteractor.configureColorPalette(ColorPalette.OceanBlue)
        fakeInteractor.configureFontFamily(AppFontFamily.Monospace)
        fakeInteractor.configureCacheEnabled(true)
        fakeInteractor.configureBaseUrl("https://example.com")

        createViewModel()
        advanceUntilIdle()

        assertThat(viewModel.state.value.themeMode).isEqualTo(ThemeMode.Dark)
        assertThat(viewModel.state.value.colorPalette).isEqualTo(ColorPalette.OceanBlue)
        assertThat(viewModel.state.value.fontFamily).isEqualTo(AppFontFamily.Monospace)
        assertThat(viewModel.state.value.isCacheEnabled).isTrue()
        assertThat(viewModel.state.value.baseUrl).isEqualTo("https://example.com")
        assertThat(viewModel.state.value.baseUrlInput).isEqualTo("https://example.com")
    }

    @Test
    fun `state updates when theme mode changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.themeModeFlow.value = ThemeMode.Light
        advanceUntilIdle()

        assertThat(viewModel.state.value.themeMode).isEqualTo(ThemeMode.Light)
    }

    @Test
    fun `state updates when color palette changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.colorPaletteFlow.value = ColorPalette.SunsetCoral
        advanceUntilIdle()

        assertThat(viewModel.state.value.colorPalette).isEqualTo(ColorPalette.SunsetCoral)
    }

    @Test
    fun `state updates when font family changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.fontFamilyFlow.value = AppFontFamily.Serif
        advanceUntilIdle()

        assertThat(viewModel.state.value.fontFamily).isEqualTo(AppFontFamily.Serif)
    }

    @Test
    fun `state updates when cache enabled changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.cacheEnabledFlow.value = false
        advanceUntilIdle()

        assertThat(viewModel.state.value.isCacheEnabled).isFalse()
    }

    @Test
    fun `selectThemeMode calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.selectThemeMode(ThemeMode.Amoled)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetThemeMode).isEqualTo(ThemeMode.Amoled)
    }

    @Test
    fun `selectColorPalette calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.selectColorPalette(ColorPalette.Forest)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetColorPalette).isEqualTo(ColorPalette.Forest)
    }

    @Test
    fun `selectFontFamily calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.selectFontFamily(AppFontFamily.System)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetFontFamily).isEqualTo(AppFontFamily.System)
    }

    @Test
    fun `setCacheEnabled calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.setCacheEnabled(false)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetCacheEnabled).isFalse()
    }

    @Test
    fun `setDirectDownloadsEnabled calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.setDirectDownloadsEnabled(true)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetDirectDownloadsEnabled).isTrue()
    }

    @Test
    fun `setExternalSearchEnabled calls interactor`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.setExternalSearchEnabled(true)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetExternalSearchEnabled).isTrue()
    }

    @Test
    fun `setFileLoggingEnabled calls interactor and refreshes log state`() = runTest {
        fakeInteractor.configureHasLogFiles(true)
        fakeInteractor.configureLogFilesSize("1.5 MB")

        createViewModel()
        advanceUntilIdle()

        viewModel.setFileLoggingEnabled(true)
        advanceUntilIdle()

        assertThat(fakeInteractor.lastSetFileLoggingEnabled).isTrue()
        assertThat(viewModel.state.value.hasLogFiles).isTrue()
        assertThat(viewModel.state.value.logFilesSize).isEqualTo("1.5 MB")
    }

    @Test
    fun `clearLogs calls interactor and refreshes state`() = runTest {
        fakeInteractor.configureHasLogFiles(true)
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.configureHasLogFiles(false)
        viewModel.clearLogs()

        assertThat(fakeInteractor.clearLogsCalled).isTrue()
        assertThat(viewModel.state.value.hasLogFiles).isFalse()
    }

    @Test
    fun `onBaseUrlInputChanged updates input without error`() = runTest {
        createViewModel()
        advanceUntilIdle()

        viewModel.onBaseUrlInputChanged("https://new-url.com")

        assertThat(viewModel.state.value.baseUrlInput).isEqualTo("https://new-url.com")
        assertThat(viewModel.state.value.baseUrlErrorRes).isNull()
    }

    @Test
    fun `saveBaseUrl does nothing when input equals current url`() = runTest {
        fakeInteractor.configureBaseUrl("https://example.com")
        createViewModel()
        advanceUntilIdle()

        viewModel.saveBaseUrl()
        advanceUntilIdle()

        assertThat(fakeInteractor.setBaseUrlCallCount).isEqualTo(0)
    }

    @Test
    fun `saveBaseUrl updates state on success`() = runTest {
        fakeInteractor.configureBaseUrl("https://old.com")
        fakeInteractor.setBaseUrlResultValue = SettingsScreenViewModel.SetBaseUrlResult.Success
        createViewModel()
        advanceUntilIdle()

        viewModel.onBaseUrlInputChanged("https://new.com")
        viewModel.saveBaseUrl()
        advanceUntilIdle()

        assertThat(viewModel.state.value.baseUrl).isEqualTo("https://new.com")
        assertThat(viewModel.state.value.baseUrlInput).isEqualTo("https://new.com")
        assertThat(viewModel.state.value.isBaseUrlSaving).isFalse()
        assertThat(viewModel.state.value.baseUrlErrorRes).isNull()
    }

    @Test
    fun `saveBaseUrl shows error on invalid url`() = runTest {
        fakeInteractor.configureBaseUrl("https://old.com")
        fakeInteractor.setBaseUrlResultValue = SettingsScreenViewModel.SetBaseUrlResult.InvalidUrl
        createViewModel()
        advanceUntilIdle()

        viewModel.onBaseUrlInputChanged("invalid-url")
        viewModel.saveBaseUrl()
        advanceUntilIdle()

        assertThat(viewModel.state.value.baseUrl).isEqualTo("https://old.com") // unchanged
        assertThat(viewModel.state.value.isBaseUrlSaving).isFalse()
        assertThat(viewModel.state.value.baseUrlErrorRes).isEqualTo(R.string.invalid_url)
    }

    @Test
    fun `state updates when storage info changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        val storageInfo = StorageInfo(
            totalBytes = 1000L,
            availableBytes = 900L,
            usedBytes = 100L,
            pressureLevel = StoragePressureLevel.LOW
        )
        fakeInteractor.storageInfoFlow.value = storageInfo
        advanceUntilIdle()

        assertThat(viewModel.state.value.storageInfo).isEqualTo(storageInfo)
    }

    @Test
    fun `state updates when direct downloads permission changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.hasIssueContentDownloadPermissionFlow.value = true
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasIssueContentDownloadPermission).isTrue()
    }

    @Test
    fun `state updates when request content permission changes`() = runTest {
        createViewModel()
        advanceUntilIdle()

        fakeInteractor.hasRequestContentPermissionFlow.value = true
        advanceUntilIdle()

        assertThat(viewModel.state.value.hasRequestContentPermission).isTrue()
    }

    private class FakeInteractor : SettingsScreenViewModel.Interactor {
        private var _themeMode = ThemeMode.Default
        private var _colorPalette = ColorPalette.Default
        private var _fontFamily = AppFontFamily.Default
        private var _cacheEnabled = true
        private var _storageInfo: StorageInfo? = null
        private var _directDownloadsEnabled = false
        private var _hasIssueContentDownloadPermission = false
        private var _externalSearchEnabled = false
        private var _hasRequestContentPermission = false
        private var _fileLoggingEnabled = false
        private var _hasLogFiles = false
        private var _logFilesSize = ""
        private var _baseUrl = ""

        val themeModeFlow = MutableStateFlow(ThemeMode.Default)
        val colorPaletteFlow = MutableStateFlow(ColorPalette.Default)
        val fontFamilyFlow = MutableStateFlow(AppFontFamily.Default)
        val cacheEnabledFlow = MutableStateFlow(true)
        val storageInfoFlow = MutableStateFlow(StorageInfo(0L, 0L, 0L, StoragePressureLevel.LOW))
        val directDownloadsEnabledFlow = MutableStateFlow(false)
        val hasIssueContentDownloadPermissionFlow = MutableStateFlow(false)
        val externalSearchEnabledFlow = MutableStateFlow(false)
        val hasRequestContentPermissionFlow = MutableStateFlow(false)
        val fileLoggingEnabledFlow = MutableStateFlow(false)

        var lastSetThemeMode: ThemeMode? = null
        var lastSetColorPalette: ColorPalette? = null
        var lastSetFontFamily: AppFontFamily? = null
        var lastSetCacheEnabled: Boolean? = null
        var lastSetDirectDownloadsEnabled: Boolean? = null
        var lastSetExternalSearchEnabled: Boolean? = null
        var lastSetFileLoggingEnabled: Boolean? = null
        var clearLogsCalled = false
        var setBaseUrlCallCount = 0
        var setBaseUrlResultValue: SettingsScreenViewModel.SetBaseUrlResult = SettingsScreenViewModel.SetBaseUrlResult.Success

        fun configureThemeMode(mode: ThemeMode) {
            _themeMode = mode
            themeModeFlow.value = mode
        }

        fun configureColorPalette(palette: ColorPalette) {
            _colorPalette = palette
            colorPaletteFlow.value = palette
        }

        fun configureFontFamily(family: AppFontFamily) {
            _fontFamily = family
            fontFamilyFlow.value = family
        }

        fun configureCacheEnabled(enabled: Boolean) {
            _cacheEnabled = enabled
            cacheEnabledFlow.value = enabled
        }

        fun configureBaseUrl(url: String) {
            _baseUrl = url
        }

        fun configureHasLogFiles(has: Boolean) {
            _hasLogFiles = has
        }

        fun configureLogFilesSize(size: String) {
            _logFilesSize = size
        }

        override fun getThemeMode(): ThemeMode = _themeMode
        override fun getColorPalette(): ColorPalette = _colorPalette
        override fun getFontFamily(): AppFontFamily = _fontFamily
        override fun isCacheEnabled(): Boolean = _cacheEnabled
        override fun getStorageInfo(): StorageInfo? = _storageInfo
        override fun isDirectDownloadsEnabled(): Boolean = _directDownloadsEnabled
        override fun hasIssueContentDownloadPermission(): Boolean = _hasIssueContentDownloadPermission
        override fun isExternalSearchEnabled(): Boolean = _externalSearchEnabled
        override fun hasRequestContentPermission(): Boolean = _hasRequestContentPermission
        override fun isFileLoggingEnabled(): Boolean = _fileLoggingEnabled
        override fun hasLogFiles(): Boolean = _hasLogFiles
        override fun getLogFilesSize(): String = _logFilesSize
        override fun getBaseUrl(): String = _baseUrl

        override fun observeThemeMode(): Flow<ThemeMode> = themeModeFlow
        override fun observeColorPalette(): Flow<ColorPalette> = colorPaletteFlow
        override fun observeFontFamily(): Flow<AppFontFamily> = fontFamilyFlow
        override fun observeCacheEnabled(): Flow<Boolean> = cacheEnabledFlow
        override fun observeStorageInfo(): Flow<StorageInfo> = storageInfoFlow
        override fun observeDirectDownloadsEnabled(): Flow<Boolean> = directDownloadsEnabledFlow
        override fun observeHasIssueContentDownloadPermission(): Flow<Boolean> = hasIssueContentDownloadPermissionFlow
        override fun observeExternalSearchEnabled(): Flow<Boolean> = externalSearchEnabledFlow
        override fun observeHasRequestContentPermission(): Flow<Boolean> = hasRequestContentPermissionFlow
        override fun observeFileLoggingEnabled(): Flow<Boolean> = fileLoggingEnabledFlow

        override suspend fun setThemeMode(themeMode: ThemeMode) {
            lastSetThemeMode = themeMode
        }

        override suspend fun setColorPalette(colorPalette: ColorPalette) {
            lastSetColorPalette = colorPalette
        }

        override suspend fun setFontFamily(fontFamily: AppFontFamily) {
            lastSetFontFamily = fontFamily
        }

        override suspend fun setCacheEnabled(enabled: Boolean) {
            lastSetCacheEnabled = enabled
        }

        override suspend fun setDirectDownloadsEnabled(enabled: Boolean): Boolean {
            lastSetDirectDownloadsEnabled = enabled
            return true
        }

        override suspend fun setExternalSearchEnabled(enabled: Boolean) {
            lastSetExternalSearchEnabled = enabled
        }

        override suspend fun setFileLoggingEnabled(enabled: Boolean) {
            lastSetFileLoggingEnabled = enabled
        }

        override fun getShareLogsIntent(): Intent = mockk()

        override fun clearLogs() {
            clearLogsCalled = true
        }

        override suspend fun setBaseUrl(url: String): SettingsScreenViewModel.SetBaseUrlResult {
            setBaseUrlCallCount++
            return setBaseUrlResultValue
        }

        override suspend fun forceSkeletonResync(): SkeletonResyncResult {
            return SkeletonResyncResult.Success
        }
    }
}
