package com.lelloman.pezzottify.android.ui.test

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreenActions
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreenEvents
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreenInternal
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreenState
import com.lelloman.pezzottify.android.ui.test.screen.LoginScreenScope
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.emptyFlow

/**
 * Main DSL scope for Pezzottify UI tests.
 *
 * This scope tests internal composables directly without Hilt,
 * treating the UI as a "puppet" controlled by state and events.
 *
 * Usage:
 * ```
 * @Test
 * fun myTest() = composeTestRule.ppiTest {
 *     givenLoginState {
 *         host = "http://localhost:3001"
 *         email = "user@example.com"
 *     }
 *
 *     launchLoginScreen()
 *
 *     onLoginScreen {
 *         clickLogin()
 *     }
 *
 *     emitLoginEvent(LoginScreenEvents.NavigateToMain)
 *     assertNavigatedTo("main")
 * }
 * ```
 */
class PezzottifyTestScope(val composeTestRule: ComposeContentTestRule) {

    // Login screen state and events
    private val loginState = mutableStateOf(LoginScreenState())
    private val loginActions = RecordingLoginActions { newState -> loginState.value = newState }

    /**
     * Configure the login screen state.
     */
    fun givenLoginState(block: LoginScreenStateBuilder.() -> Unit) {
        loginState.value = LoginScreenStateBuilder().apply(block).build()
    }

    /**
     * Launch the login screen for testing.
     * Tests the internal composable directly without navigation.
     */
    fun launchLoginScreen() {
        composeTestRule.setContent {
            val navController = rememberNavController()
            PezzottifyTheme {
                LoginScreenInternal(
                    state = loginState.value,
                    events = emptyFlow(),
                    actions = loginActions,
                    navController = navController
                )
            }
        }

        composeTestRule.waitForIdle()
    }

    /**
     * Access login screen for interactions and assertions.
     */
    fun onLoginScreen(block: LoginScreenScope.() -> Unit) {
        LoginScreenScope(composeTestRule).apply(block)
    }

    /**
     * Wait for idle state (all recompositions complete).
     */
    fun waitForIdle() {
        composeTestRule.waitForIdle()
    }

    /**
     * Get recorded actions for verification.
     */
    fun getLoginActions(): RecordingLoginActions = loginActions

    /**
     * Verify that login was clicked.
     */
    fun verifyLoginClicked() {
        if (loginActions.loginClickCount == 0) {
            throw AssertionError("Expected login button to be clicked, but it was not")
        }
    }
}

/**
 * Builder for LoginScreenState.
 */
class LoginScreenStateBuilder {
    var host: String = ""
    var email: String = ""
    var password: String = ""
    var isLoading: Boolean = false
    var hostError: String? = null
    var emailError: String? = null
    var error: String? = null

    fun build() = LoginScreenState(
        host = host,
        email = email,
        password = password,
        isLoading = isLoading,
        hostError = hostError,
        emailError = emailError,
        error = error
    )
}

/**
 * Recording implementation of LoginScreenActions that tracks calls.
 */
class RecordingLoginActions(
    private val onStateChange: (LoginScreenState) -> Unit
) : LoginScreenActions {

    private var currentState = LoginScreenState()

    val hostUpdates = mutableListOf<String>()
    val emailUpdates = mutableListOf<String>()
    val passwordUpdates = mutableListOf<String>()
    var loginClickCount = 0
        private set

    override fun updateHost(host: String) {
        hostUpdates.add(host)
        currentState = currentState.copy(host = host)
        onStateChange(currentState)
    }

    override fun updateEmail(email: String) {
        emailUpdates.add(email)
        currentState = currentState.copy(email = email)
        onStateChange(currentState)
    }

    override fun updatePassword(password: String) {
        passwordUpdates.add(password)
        currentState = currentState.copy(password = password)
        onStateChange(currentState)
    }

    override fun clockOnLoginButton() {
        loginClickCount++
    }
}

/**
 * Entry point for the test DSL.
 */
fun ComposeContentTestRule.ppiTest(block: PezzottifyTestScope.() -> Unit) {
    PezzottifyTestScope(this).apply(block)
}
