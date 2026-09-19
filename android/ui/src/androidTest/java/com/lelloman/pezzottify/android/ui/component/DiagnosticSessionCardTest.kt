package com.lelloman.pezzottify.android.ui.component

import android.graphics.Bitmap
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.platform.app.InstrumentationRegistry
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import java.io.File

class DiagnosticSessionCardTest {
    @get:Rule val compose = createComposeRule()
    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test fun inactiveLightCardShowsExplanationAndOpensOnlyWhenClicked() {
        var clicks = 0
        compose.setContent {
            PezzottifyTheme(darkTheme = false) {
                Surface { DiagnosticSessionCard(false, null, { clicks++ }, Modifier.width(360.dp)) }
            }
        }
        compose.onNodeWithText(context.getString(R.string.diagnostic_session_inactive)).assertIsDisplayed()
        compose.onNodeWithText(context.getString(R.string.diagnostic_session_privacy)).assertIsDisplayed()
        assertEquals(0, clicks)
        screenshot("diagnostics-light.png")
        compose.onNodeWithText(context.getString(R.string.open_diagnostic_session)).performClick()
        compose.runOnIdle { assertEquals(1, clicks) }
    }

    @Test fun activeDarkCardShowsExpiryAndManageAction() {
        compose.setContent {
            PezzottifyTheme(darkTheme = true) {
                Surface { DiagnosticSessionCard(true, 12, {}, Modifier.width(360.dp)) }
            }
        }
        compose.onNodeWithText(context.getString(R.string.diagnostic_session_remaining, 12L)).assertIsDisplayed()
        compose.onNodeWithText(context.getString(R.string.manage_diagnostic_session)).assertHasClickAction()
        screenshot("diagnostics-dark.png")
    }

    private fun screenshot(name: String) {
        val bitmap = compose.onRoot().captureToImage().asAndroidBitmap()
        File(context.getExternalFilesDir(null), name).outputStream().use {
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, it)
        }
    }
}
