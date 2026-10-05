package com.lelloman.pezzottify.android.ui.screen.player

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SnackbarHostState
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.navigation.compose.rememberNavController
import io.mockk.mockk
import io.mockk.verify
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "en-w411dp-h891dp")
class LyricsPlayerScreenTest {
    @get:Rule val compose = createComposeRule()
    private val actions = mockk<PlayerScreenActions>(relaxed = true)
    private val playing = PlayerScreenState(isLoading = false, trackId = "t", trackName = "Song", trackDurationSec = 100, hasNextTrack = true, hasPreviousTrack = true)
    private val lyrics = PlayerLyricsState("t", lines = (0..20).map { LyricLine(it * 5000L, "Line $it") })

    @Test fun `lyrics icon opens focused player and back returns without affecting playback`() {
        compose.setContent {
            MaterialTheme {
                PlayerScreenContent(playing, actions, rememberNavController(), remember { SnackbarHostState() }, lyrics)
            }
        }
        compose.onNodeWithContentDescription("Open lyrics player").performScrollTo().performClick()
        compose.onNodeWithContentDescription("Back to player").assertIsDisplayed()
        compose.onNodeWithContentDescription("Next").performClick()
        verify { actions.clickOnSkipNext() }
        compose.onNodeWithContentDescription("Play").performClick()
        verify { actions.clickOnPlayPause() }
        compose.onNodeWithText("Line 0").assertIsDisplayed()
        compose.onNodeWithContentDescription("Back to player").performClick()
        compose.onNodeWithContentDescription("Open lyrics player").performScrollTo().assertIsDisplayed()
        verify(exactly = 1) { actions.clickOnPlayPause() }
    }

    @Test fun `icon is hidden for missing or previous tracks lyrics including radio`() {
        val data = mutableStateOf(PlayerLyricsState())
        compose.setContent {
            MaterialTheme {
                PlayerScreenContent(playing.copy(isRadio = true), actions, rememberNavController(), remember { SnackbarHostState() }, data.value)
            }
        }
        compose.onNodeWithContentDescription("Open lyrics player").assertDoesNotExist()
        compose.runOnIdle { data.value = lyrics.copy(trackId = "old") }
        compose.onNodeWithContentDescription("Open lyrics player").assertDoesNotExist()
        compose.runOnIdle { data.value = lyrics }
        compose.onNodeWithContentDescription("Open lyrics player").performScrollTo().assertIsDisplayed()
    }

    @Test fun `focused view updates tracks without displaying stale lyrics`() {
        val state = mutableStateOf(playing)
        compose.setContent { MaterialTheme { LyricsPlayerScreen(state.value, lyrics, actions, {}) } }
        compose.onNodeWithText("Line 0").assertIsDisplayed()
        compose.runOnIdle { state.value = playing.copy(trackId = "new", trackName = "Next song") }
        compose.onNodeWithText("Next song").assertIsDisplayed()
        compose.onNodeWithText("Line 0").assertDoesNotExist()
        compose.onNodeWithText("No lyrics available for this track.").assertIsDisplayed()
        compose.onNodeWithContentDescription("Back to player").assertIsDisplayed()
    }
}
