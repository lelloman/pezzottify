package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import com.lelloman.pezzottify.android.domain.equalizer.*
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import io.mockk.mockk
import io.mockk.every
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
            profiles = listOf(profile), selectedProfileId = profile.id), profilesOnly = true)
        compose.onNodeWithText("Sony Headphones · Modified").assertExists()
        compose.onNodeWithText("Save changes to Sony Headphones").performScrollTo().performClick()
        verify { actions.updateProfile() }
        compose.onNodeWithText("Delete profile").performScrollTo().performClick()
        verify(exactly = 0) { actions.deleteProfile(any()) }
        compose.onNodeWithText("Delete “Sony Headphones”? Your current sound settings will be kept.").assertExists()
        compose.onAllNodesWithText("Delete profile").filter(hasClickAction()).onLast().performClick()
        verify { actions.deleteProfile("sony") }
    }

    @Test fun `band labels flank the slider in one compact row`() {
        show(EqualizerSettings())
        compose.onNodeWithContentDescription("31 Hz").performScrollTo()
        val frequency = compose.onNodeWithText("31 Hz").getUnclippedBoundsInRoot()
        val slider = compose.onNodeWithContentDescription("31 Hz").getUnclippedBoundsInRoot()
        val gain = compose.onAllNodesWithText("+0.0 dB").onFirst().getUnclippedBoundsInRoot()
        // Slider semantics include an expanded touch target; compare visual ordering by centers.
        org.junit.Assert.assertTrue(frequency.left + frequency.right < slider.left + slider.right)
        org.junit.Assert.assertTrue(slider.left + slider.right < gain.left + gain.right)
        org.junit.Assert.assertEquals(frequency.top, gain.top)
    }

    @Test fun `profiles screen associates using device identity`() {
        val associate = mockk<(String, String) -> Boolean>(relaxed = true)
        every { associate(any(), any()) } returns true
        val profile = EqualizerProfile("sony", "Sony EQ", EqualizerBands.flat)
        compose.setContent { PezzottifyTheme {
            EqualizerProfilesContent(EqualizerSettings(profiles = listOf(profile)), actions,
                EqualizerOutput("bluetooth:AA:BB:CC:DD:EE:FF", "Sony Headphones", bluetooth = true), associate)
        } }
        compose.onNodeWithText("Associate with current output: Sony Headphones").performScrollTo().performClick()
        verify { associate("sony", "bluetooth:AA:BB:CC:DD:EE:FF") }
    }

    @Test fun `existing association can be removed`() {
        val profile = EqualizerProfile("sony", "Sony EQ", EqualizerBands.flat)
        compose.setContent { PezzottifyTheme {
            EqualizerProfilesContent(EqualizerSettings(profiles = listOf(profile), outputAssociations = listOf(
                EqualizerOutputAssociation("bt", "Sony", "sony"))), actions,
                EqualizerOutput("bt", "Sony", bluetooth = true), { _, _ -> true })
        } }
        compose.onNodeWithText("Associated with Sony").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithText("Remove association: Sony").performScrollTo().performClick()
        verify { actions.removeOutputAssociation("bt") }
    }

    @Test fun `unidentified Bluetooth output offers permission and disables association`() {
        val request = mockk<() -> Unit>(relaxed = true)
        compose.setContent { PezzottifyTheme {
            EqualizerProfilesContent(EqualizerSettings(profiles = listOf(EqualizerProfile("p", "EQ", EqualizerBands.flat))), actions,
                EqualizerOutput(null, "Sony", true, true), { _, _ -> false }, request)
        } }
        compose.onNodeWithText("Allow Bluetooth device access").performScrollTo().performClick()
        verify { request() }
        compose.onNodeWithText("Associate with current output: Sony").performScrollTo().assertIsNotEnabled()
    }

    private fun show(state: EqualizerSettings, profilesOnly: Boolean = false) {
        compose.setContent { PezzottifyTheme { EqualizerContent(state, actions, profilesOnly = profilesOnly) } }
    }
}
