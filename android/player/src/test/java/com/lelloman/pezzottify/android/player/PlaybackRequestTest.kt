package com.lelloman.pezzottify.android.player

import androidx.media3.common.Player
import org.junit.Assert.assertEquals
import org.junit.Test
import java.lang.reflect.Proxy

class PlaybackRequestTest {
    /** Unexpected calls fail, including queue mutations or seeks that would lose the position. */
    private class FakePlayer(val state: Int, val count: Int) {
        val calls = mutableListOf<String>()
        val player = Proxy.newProxyInstance(
            Player::class.java.classLoader, arrayOf(Player::class.java)
        ) { _, method, args ->
            when (method.name) {
                "getPlaybackState" -> state
                "getMediaItemCount" -> count
                "prepare" -> { calls.add("prepare"); null }
                "setPlayWhenReady" -> { calls.add("playWhenReady=${args!![0]}"); null }
                else -> error("Unexpected player call: ${method.name}")
            }
        } as Player
    }

    @Test fun idleLoadedPlayPreparesBeforeSettingIntent() {
        val fake = FakePlayer(Player.STATE_IDLE, 21)
        fake.player.requestPlayback(true) { fake.calls.add("diagnostic") }
        assertEquals(listOf("diagnostic", "prepare", "playWhenReady=true"), fake.calls)
    }

    @Test fun pauseNeverPrepares() {
        for (state in listOf(Player.STATE_IDLE, Player.STATE_BUFFERING, Player.STATE_READY, Player.STATE_ENDED)) {
            val fake = FakePlayer(state, 21)
            fake.player.requestPlayback(false)
            assertEquals(listOf("playWhenReady=false"), fake.calls)
        }
    }

    @Test fun emptyIdleQueueDoesNotPrepare() {
        val fake = FakePlayer(Player.STATE_IDLE, 0)
        fake.player.requestPlayback(true)
        assertEquals(listOf("playWhenReady=true"), fake.calls)
    }

    @Test fun preparedPlayerDoesNotPrepareAgain() {
        for (state in listOf(Player.STATE_BUFFERING, Player.STATE_READY, Player.STATE_ENDED)) {
            val fake = FakePlayer(state, 21)
            fake.player.requestPlayback(true)
            assertEquals(listOf("playWhenReady=true"), fake.calls)
        }
    }
}
