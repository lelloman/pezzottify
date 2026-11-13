package com.lelloman.pezzottify.android.updates

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject

/**
 * LelloStore asks before an automatic update, which kills the process and with it the music
 * playing on this device. It keeps asking until this stops answering busy, and gives up after a
 * day. See lellostore's docs/APP_UPDATE_CONSENT.md.
 *
 * Only local playback counts: paused playback is saved and restored after the update, and
 * controlling another device keeps playing there.
 */
@AndroidEntryPoint
class UpdateConsentReceiver : BroadcastReceiver() {
    @Inject
    lateinit var platformPlayer: PlatformPlayer

    override fun onReceive(context: Context, intent: Intent) {
        if (platformPlayer.isPlaying.value) resultCode = RESULT_BUSY
    }

    private companion object {
        const val RESULT_BUSY = 1
    }
}
