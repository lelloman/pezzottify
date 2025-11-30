package com.lelloman.pezzottify.android.ui.screen.login

import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.test.junit4.createComposeRule
import com.lelloman.pezzottify.android.ui.test.ppiTest
import org.junit.Rule
import org.junit.Test

class LoginScreenTest {

    @get:Rule
    val composeTestRule: ComposeContentTestRule = createComposeRule()

    @Test
    fun loginScreen_displaysAllFields() = composeTestRule.ppiTest {
        givenLoginState {
            host = "http://localhost:3001"
        }

        launchLoginScreen()

        onLoginScreen {
            shouldBeDisplayed()
            shouldShowServerUrl("http://localhost:3001")
        }
    }

    @Test
    fun loginScreen_recordsLoginClick() = composeTestRule.ppiTest {
        givenLoginState {
            host = "http://localhost:3001"
        }

        launchLoginScreen()

        onLoginScreen {
            typeEmail("user@example.com")
            typePassword("secretpassword")
            clickLogin()
        }

        // Verify the action was recorded
        verifyLoginClicked()
    }

    @Test
    fun loginScreen_showsHostError_whenStateHasHostError() = composeTestRule.ppiTest {
        givenLoginState {
            host = "http://localhost:3001"
            hostError = "Invalid URL"
        }

        launchLoginScreen()

        onLoginScreen {
            shouldShowError("Invalid URL")
        }
    }

    @Test
    fun loginScreen_displaysFields_whenLoading() = composeTestRule.ppiTest {
        givenLoginState {
            host = "http://localhost:3001"
            isLoading = true
        }

        launchLoginScreen()

        onLoginScreen {
            shouldBeDisplayed()
        }
    }
}
