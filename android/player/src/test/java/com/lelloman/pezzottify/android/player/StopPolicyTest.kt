package com.lelloman.pezzottify.android.player

import android.os.Bundle
import androidx.media3.common.Player
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class StopPolicyTest {

    private val own = "com.lelloman.pezzottify.android"

    @Test
    fun `only the app controller in our package is recognised`() {
        assertTrue(StopPolicy.isAppController(own, own, StopPolicy.appControllerHints()))
        // The media notification controller and legacy callers share our package, without the hint.
        assertFalse(StopPolicy.isAppController(own, own, Bundle()))
        assertFalse(StopPolicy.isAppController("com.android.systemui", own, StopPolicy.appControllerHints()))
    }

    @Test
    fun `stop from anything but the app controller becomes a pause`() {
        assertTrue(StopPolicy.stopBecomesPause(Player.COMMAND_STOP, fromAppController = false))
        assertFalse(StopPolicy.stopBecomesPause(Player.COMMAND_STOP, fromAppController = true))
        assertFalse(StopPolicy.stopBecomesPause(Player.COMMAND_PLAY_PAUSE, fromAppController = false))
        assertFalse(StopPolicy.stopBecomesPause(Player.COMMAND_PREPARE, fromAppController = false))
    }
}
