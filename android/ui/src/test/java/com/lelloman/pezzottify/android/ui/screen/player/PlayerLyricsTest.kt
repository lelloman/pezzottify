package com.lelloman.pezzottify.android.ui.screen.player

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.google.common.truth.Truth.assertThat
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "en")
class PlayerLyricsTest {
    @get:Rule val compose = createComposeRule()

    @Test fun `current lyric follows playback and backward seeks and clicking seeks in percent`() {
        val position = mutableIntStateOf(2)
        var seek = -1f
        val lyrics = PlayerLyricsState("t", lines = (0..20).map { LyricLine(it * 5000L, "Line $it") })
        compose.setContent { MaterialTheme { PlayerLyrics(lyrics, position.intValue, 100) { seek = it } } }
        compose.onNodeWithText("Line 0").assertIsSelected()
        compose.runOnIdle { position.intValue = 40 }
        compose.waitForIdle()
        compose.onNodeWithText("Line 8").assertIsDisplayed().assertIsSelected().performClick()
        assertThat(seek).isEqualTo(40f)
        compose.runOnIdle { position.intValue = 1 }
        compose.waitForIdle()
        compose.onNodeWithText("Line 0").assertIsDisplayed().assertIsSelected()
    }

    @Test fun `manual scrolling exposes a way to resume following`() {
        val lyrics = PlayerLyricsState("t", lines = (0..20).map { LyricLine(it * 5000L, "Line $it") })
        compose.setContent { MaterialTheme { PlayerLyrics(lyrics, 1, 100) {} } }
        compose.onNode(hasScrollAction()).performTouchInput { swipeUp() }
        compose.onNodeWithText("Follow current line").assertIsDisplayed().performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Line 0").assertIsDisplayed().assertIsSelected()
    }

    @Test fun `plain lyrics are displayed without timed controls`() {
        compose.setContent { MaterialTheme { PlayerLyrics(PlayerLyricsState("t", plainText = "Plain lyrics\nAnother line"), 10, 100) {} } }
        compose.onNodeWithText("Plain lyrics\nAnother line").assertIsDisplayed()
        compose.onNodeWithText("Follow current line").assertDoesNotExist()
    }
}
