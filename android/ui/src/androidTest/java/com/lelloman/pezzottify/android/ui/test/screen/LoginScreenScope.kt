package com.lelloman.pezzottify.android.ui.test.screen

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput

/**
 * DSL scope for interacting with and asserting on the Login screen.
 */
class LoginScreenScope(private val rule: ComposeContentTestRule) {

    // === Actions ===

    fun typeServerUrl(url: String) {
        rule.onNodeWithText("Server URL").performTextClearance()
        rule.onNodeWithText("Server URL").performTextInput(url)
    }

    fun typeEmail(email: String) {
        rule.onNodeWithText("Email").performTextInput(email)
    }

    fun typePassword(password: String) {
        rule.onNodeWithText("Password").performTextInput(password)
    }

    fun clearEmail() {
        rule.onNodeWithText("Email").performTextClearance()
    }

    fun clearPassword() {
        rule.onNodeWithText("Password").performTextClearance()
    }

    fun clickLogin() {
        rule.onNodeWithText("Login").performClick()
    }

    // === Assertions ===

    fun shouldShowServerUrl(url: String) {
        rule.onNode(hasText(url)).assertIsDisplayed()
    }

    fun shouldShowEmail(email: String) {
        rule.onNode(hasText(email)).assertIsDisplayed()
    }

    fun shouldShowLoginButton() {
        rule.onNodeWithText("Login").assertIsDisplayed()
    }

    fun shouldHaveLoginButtonEnabled() {
        rule.onNodeWithText("Login").assertIsEnabled()
    }

    fun shouldHaveLoginButtonDisabled() {
        rule.onNodeWithText("Login").assertIsNotEnabled()
    }

    fun shouldShowError(message: String) {
        rule.onNodeWithText(message).assertIsDisplayed()
    }

    fun shouldBeDisplayed() {
        rule.onNodeWithText("Email").assertIsDisplayed()
        rule.onNodeWithText("Password").assertIsDisplayed()
        rule.onNodeWithText("Login").assertIsDisplayed()
    }
}
