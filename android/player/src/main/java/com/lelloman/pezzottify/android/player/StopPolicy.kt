package com.lelloman.pezzottify.android.player

import android.os.Bundle
import androidx.media3.common.Player

/**
 * Only the app's own controller may stop local playback; any other STOP becomes a pause.
 *
 * Stops nobody asked for left the player idle (LLPR/PEZZOTTIFY-14): one arrived 10 minutes
 * into a pause, when Media3's media notification stops being ongoing, and another half a
 * second after Play had prepared the player again, so that Play was lost. Both came from a
 * controller in our own package that was not the app's. A paused player resumes on Play.
 */
internal object StopPolicy {
    /** Connection hint set only by the app's own MediaController (ExoPlatformPlayer). */
    const val APP_CONTROLLER_HINT = "com.lelloman.pezzottify.android.APP_CONTROLLER"

    fun appControllerHints(): Bundle = Bundle().apply { putBoolean(APP_CONTROLLER_HINT, true) }

    fun isAppController(callerPackage: String, ownPackage: String, hints: Bundle): Boolean =
        callerPackage == ownPackage && hints.getBoolean(APP_CONTROLLER_HINT, false)

    fun stopBecomesPause(command: Int, fromAppController: Boolean): Boolean =
        command == Player.COMMAND_STOP && !fromAppController
}
