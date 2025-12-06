package com.lelloman.pezzottify.android.localdata.internal.settings

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.settings.AppFontFamily
import com.lelloman.pezzottify.android.domain.settings.ColorPalette
import com.lelloman.pezzottify.android.domain.settings.PlayBehavior
import com.lelloman.pezzottify.android.domain.settings.ThemeMode
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class UserSettingsStoreImplTest {

    private lateinit var context: Context
    private val testDispatcher = StandardTestDispatcher()

    @Before
    fun setup() {
        context = ApplicationProvider.getApplicationContext()
        // Clear shared preferences before each test
        context.getSharedPreferences(
            UserSettingsStoreImpl.SHARED_PREF_FILE_NAME,
            Context.MODE_PRIVATE
        ).edit().clear().commit()
    }

    @Test
    fun `playBehavior returns default value when not set`() {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store.playBehavior.value).isEqualTo(PlayBehavior.Default)
        assertThat(store.playBehavior.value).isEqualTo(PlayBehavior.ReplacePlaylist)
    }

    @Test
    fun `themeMode returns default value when not set`() {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Default)
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.System)
    }

    @Test
    fun `setPlayBehavior persists value`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setPlayBehavior(PlayBehavior.AddToPlaylist)

        assertThat(store.playBehavior.value).isEqualTo(PlayBehavior.AddToPlaylist)
    }

    @Test
    fun `setThemeMode persists value`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setThemeMode(ThemeMode.Dark)

        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Dark)
    }

    @Test
    fun `playBehavior value survives store recreation`() = runTest(testDispatcher) {
        val store1 = UserSettingsStoreImpl(context, testDispatcher)
        store1.setPlayBehavior(PlayBehavior.AddToPlaylist)

        val store2 = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store2.playBehavior.value).isEqualTo(PlayBehavior.AddToPlaylist)
    }

    @Test
    fun `themeMode value survives store recreation`() = runTest(testDispatcher) {
        val store1 = UserSettingsStoreImpl(context, testDispatcher)
        store1.setThemeMode(ThemeMode.Light)

        val store2 = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store2.themeMode.value).isEqualTo(ThemeMode.Light)
    }

    @Test
    fun `setting all theme modes works correctly`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setThemeMode(ThemeMode.System)
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.System)

        store.setThemeMode(ThemeMode.Light)
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Light)

        store.setThemeMode(ThemeMode.Dark)
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Dark)

        store.setThemeMode(ThemeMode.Amoled)
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Amoled)
    }

    @Test
    fun `setting both play behaviors works correctly`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setPlayBehavior(PlayBehavior.ReplacePlaylist)
        assertThat(store.playBehavior.value).isEqualTo(PlayBehavior.ReplacePlaylist)

        store.setPlayBehavior(PlayBehavior.AddToPlaylist)
        assertThat(store.playBehavior.value).isEqualTo(PlayBehavior.AddToPlaylist)
    }

    @Test
    fun `invalid stored value falls back to default`() {
        // Manually write invalid value
        context.getSharedPreferences(
            UserSettingsStoreImpl.SHARED_PREF_FILE_NAME,
            Context.MODE_PRIVATE
        ).edit()
            .putString(UserSettingsStoreImpl.KEY_PLAY_BEHAVIOR, "InvalidValue")
            .putString(UserSettingsStoreImpl.KEY_THEME_MODE, "InvalidValue")
            .putString(UserSettingsStoreImpl.KEY_COLOR_PALETTE, "InvalidValue")
            .putString(UserSettingsStoreImpl.KEY_FONT_FAMILY, "InvalidValue")
            .commit()

        val store = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store.playBehavior.value).isEqualTo(PlayBehavior.Default)
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Default)
        assertThat(store.colorPalette.value).isEqualTo(ColorPalette.Default)
        assertThat(store.fontFamily.value).isEqualTo(AppFontFamily.Default)
    }

    @Test
    fun `colorPalette returns default value when not set`() {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store.colorPalette.value).isEqualTo(ColorPalette.Default)
        assertThat(store.colorPalette.value).isEqualTo(ColorPalette.Classic)
    }

    @Test
    fun `fontFamily returns default value when not set`() {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store.fontFamily.value).isEqualTo(AppFontFamily.Default)
        assertThat(store.fontFamily.value).isEqualTo(AppFontFamily.System)
    }

    @Test
    fun `setColorPalette persists value`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setColorPalette(ColorPalette.OceanBlue)

        assertThat(store.colorPalette.value).isEqualTo(ColorPalette.OceanBlue)
    }

    @Test
    fun `setFontFamily persists value`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setFontFamily(AppFontFamily.Monospace)

        assertThat(store.fontFamily.value).isEqualTo(AppFontFamily.Monospace)
    }

    @Test
    fun `colorPalette value survives store recreation`() = runTest(testDispatcher) {
        val store1 = UserSettingsStoreImpl(context, testDispatcher)
        store1.setColorPalette(ColorPalette.PurpleHaze)

        val store2 = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store2.colorPalette.value).isEqualTo(ColorPalette.PurpleHaze)
    }

    @Test
    fun `fontFamily value survives store recreation`() = runTest(testDispatcher) {
        val store1 = UserSettingsStoreImpl(context, testDispatcher)
        store1.setFontFamily(AppFontFamily.Serif)

        val store2 = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store2.fontFamily.value).isEqualTo(AppFontFamily.Serif)
    }

    @Test
    fun `setting all color palettes works correctly`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        ColorPalette.entries.forEach { palette ->
            store.setColorPalette(palette)
            assertThat(store.colorPalette.value).isEqualTo(palette)
        }
    }

    @Test
    fun `setting all font families works correctly`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        AppFontFamily.entries.forEach { fontFamily ->
            store.setFontFamily(fontFamily)
            assertThat(store.fontFamily.value).isEqualTo(fontFamily)
        }
    }

    @Test
    fun `legacy AmoledBlack palette migrates to Amoled theme mode`() {
        // Manually write the legacy AmoledBlack value
        context.getSharedPreferences(
            UserSettingsStoreImpl.SHARED_PREF_FILE_NAME,
            Context.MODE_PRIVATE
        ).edit()
            .putString(UserSettingsStoreImpl.KEY_COLOR_PALETTE, "AmoledBlack")
            .putString(UserSettingsStoreImpl.KEY_THEME_MODE, "System")
            .commit()

        val store = UserSettingsStoreImpl(context, testDispatcher)

        // Should migrate to Amoled theme mode and Classic palette
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Amoled)
        assertThat(store.colorPalette.value).isEqualTo(ColorPalette.Classic)
    }

    @Test
    fun `normal palette does not trigger migration`() {
        // Write a normal palette value
        context.getSharedPreferences(
            UserSettingsStoreImpl.SHARED_PREF_FILE_NAME,
            Context.MODE_PRIVATE
        ).edit()
            .putString(UserSettingsStoreImpl.KEY_COLOR_PALETTE, "OceanBlue")
            .putString(UserSettingsStoreImpl.KEY_THEME_MODE, "Dark")
            .commit()

        val store = UserSettingsStoreImpl(context, testDispatcher)

        // Should keep the stored values
        assertThat(store.themeMode.value).isEqualTo(ThemeMode.Dark)
        assertThat(store.colorPalette.value).isEqualTo(ColorPalette.OceanBlue)
    }

    // region directDownloadsEnabled

    @Test
    fun `directDownloadsEnabled returns false by default`() {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store.directDownloadsEnabled.value).isFalse()
    }

    @Test
    fun `setDirectDownloadsEnabled persists value`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)

        store.setDirectDownloadsEnabled(true)

        assertThat(store.directDownloadsEnabled.value).isTrue()
    }

    @Test
    fun `directDownloadsEnabled value survives store recreation`() = runTest(testDispatcher) {
        val store1 = UserSettingsStoreImpl(context, testDispatcher)
        store1.setDirectDownloadsEnabled(true)

        val store2 = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store2.directDownloadsEnabled.value).isTrue()
    }

    @Test
    fun `clearSyncedSettings resets directDownloadsEnabled to default`() = runTest(testDispatcher) {
        val store = UserSettingsStoreImpl(context, testDispatcher)
        store.setDirectDownloadsEnabled(true)

        store.clearSyncedSettings()

        assertThat(store.directDownloadsEnabled.value).isFalse()
    }

    @Test
    fun `clearSyncedSettings persists reset value`() = runTest(testDispatcher) {
        val store1 = UserSettingsStoreImpl(context, testDispatcher)
        store1.setDirectDownloadsEnabled(true)
        store1.clearSyncedSettings()

        val store2 = UserSettingsStoreImpl(context, testDispatcher)

        assertThat(store2.directDownloadsEnabled.value).isFalse()
    }

    // endregion
}
