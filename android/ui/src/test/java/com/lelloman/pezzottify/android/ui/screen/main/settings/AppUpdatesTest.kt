package com.lelloman.pezzottify.android.ui.screen.main.settings

import android.content.ComponentName
import android.content.pm.ActivityInfo
import com.google.common.truth.Truth.assertThat
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], manifest = Config.NONE)
class AppUpdatesTest {
    private val context = RuntimeEnvironment.getApplication()
    private val activity = "com.lelloman.paravoidandroid.delivery.ShellUpdatesActivity"

    @Test
    fun ordinaryAppWithoutShellHasNoUpdateButtonCapability() {
        assertThat(AppUpdates.available(context)).isFalse()
        assertThat(AppUpdates.open(context)).isFalse()
        assertThat(shadowOf(context).nextStartedActivity).isNull()
    }

    @Test
    fun opensPrivateShellControlsWithoutAnExportedLauncherAlias() {
        registerControls(enabled = true)
        assertThat(AppUpdates.available(context)).isTrue()
        assertThat(AppUpdates.open(context)).isTrue()
        assertThat(shadowOf(context).nextStartedActivity.component)
            .isEqualTo(ComponentName(context.packageName, activity))
    }

    @Test
    fun disabledControlsAreUnavailable() {
        registerControls(enabled = false)
        assertThat(AppUpdates.available(context)).isFalse()
        assertThat(AppUpdates.open(context)).isFalse()
    }

    private fun registerControls(enabled: Boolean) {
        shadowOf(context.packageManager).addOrUpdateActivity(ActivityInfo().apply {
            name = activity
            packageName = context.packageName
            applicationInfo = context.applicationInfo
            this.enabled = enabled
            exported = false
        })
    }
}
