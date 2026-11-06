package com.lelloman.pezzottify.android.ui.screen.main.devices

import android.content.Context
import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.R
import io.mockk.every
import io.mockk.mockk
import org.junit.Test

class PlaybackErrorMessagesTest {

    private val context = mockk<Context> {
        every { getString(R.string.playback_error_forbidden) } returns "forbidden text"
        every { getString(R.string.playback_error_device_not_found) } returns "not found text"
        every { getString(R.string.playback_error_unknown_command) } returns "unknown text"
        every { getString(R.string.playback_error_generic) } returns "generic text"
    }

    @Test
    fun `known codes use our own wording rather than the server's`() {
        assertThat(playbackErrorMessage(context, "forbidden", "raw")).isEqualTo("forbidden text")
        assertThat(playbackErrorMessage(context, "device_not_found", "raw")).isEqualTo("not found text")
        assertThat(playbackErrorMessage(context, "unknown_command", "raw")).isEqualTo("unknown text")
    }

    @Test
    fun `other codes show the server message and fall back when it is blank`() {
        assertThat(playbackErrorMessage(context, "command_failed", "raw")).isEqualTo("raw")
        assertThat(playbackErrorMessage(context, "command_failed", " ")).isEqualTo("generic text")
    }
}
