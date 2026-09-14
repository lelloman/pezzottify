package com.lelloman.pezzottify.android.player

import androidx.media3.common.Player

/** LLPR/PEZZOTTIFY-14: playWhenReady alone cannot restart a stopped/failed idle player. */
internal fun Player.requestPlayback(playing: Boolean, beforePrepare: () -> Unit = {}) {
    if (playing && playbackState == Player.STATE_IDLE && mediaItemCount > 0) {
        beforePrepare()
        prepare()
    }
    // Do not reload or seek: prepare preserves the queue and resume position.
    playWhenReady = playing
}
