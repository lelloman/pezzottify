package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import androidx.annotation.StringRes

data class BugReportScreenState(
    val title: String = "",
    val description: String = "",
    val includeLogs: Boolean = true,
    val isSubmitting: Boolean = false,
    @StringRes val errorRes: Int? = null,
    val submitResult: SubmitResult? = null,
)

sealed interface SubmitResult {
    data object Success : SubmitResult
    data class Error(val message: String) : SubmitResult
}
