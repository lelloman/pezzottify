package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.lelloman.pezzottify.android.domain.equalizer.*
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import io.mockk.mockk
import io.mockk.verify
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35], qualifiers = "en")
class EqualizerScreenTest {
    @get:Rule val compose = createComposeRule()
    private val actions = mockk<EqualizerStore>(relaxed = true)

    @Test fun `enable switch and band controls are accessible`() {
        show(EqualizerSettings())
        compose.onNodeWithContentDescription("Equalizer").assertIsOff().performClick()
        verify { actions.setEnabled(true) }
        EqualizerBands.frequencies.forEach { hz ->
            val label = if (hz < 1000) "$hz Hz" else "${hz / 1000f} kHz"
            compose.onNodeWithContentDescription(label).performScrollTo().assertExists()
        }
        compose.onNodeWithText("Reset to flat").performScrollTo().performClick()
        verify { actions.reset() }
    }

    @Test fun `save dialog rejects blank and duplicate profile names`() {
        show(EqualizerSettings(profiles = listOf(EqualizerProfile("speaker", "Speaker", EqualizerBands.flat))))
        compose.onNodeWithText("Save as new profile").performScrollTo().performClick()
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Profile name").performTextInput(" speaker ")
        compose.onNodeWithText("A profile with this name already exists.").assertExists()
        compose.onNodeWithText("Save").assertIsNotEnabled()
        compose.onNodeWithText("Profile name").performTextReplacement("Sony Headphones")
        compose.onNodeWithText("Save").performClick()
        verify { actions.saveProfile("Sony Headphones") }
    }

    @Test fun `modified profile is explicit and deletion requires confirmation`() {
        val profile = EqualizerProfile("sony", "Sony Headphones", EqualizerBands.flat)
        show(EqualizerSettings(gains = EqualizerBands.flat.toMutableList().also { it[0] = 3f },
            profiles = listOf(profile), selectedProfileId = profile.id))
        compose.onNodeWithText("Sony Headphones · Modified").assertExists()
        compose.onNodeWithText("Save changes to Sony Headphones").performScrollTo().performClick()
        verify { actions.updateProfile() }
        compose.onNodeWithText("Delete profile").performScrollTo().performClick()
        verify(exactly = 0) { actions.deleteProfile(any()) }
        compose.onNodeWithText("Delete “Sony Headphones”? Your current sound settings will be kept.").assertExists()
        compose.onAllNodesWithText("Delete profile").filter(hasClickAction()).onLast().performClick()
        verify { actions.deleteProfile("sony") }
    }

    private fun show(state: EqualizerSettings) {
        compose.setContent { PezzottifyTheme { EqualizerContent(state, actions) } }
    }
}
