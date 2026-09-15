package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import androidx.annotation.StringRes

data class BugReportScreenState(
    val title: String = "",
    val description: String = "",
    val includeLogs: Boolean = false,
    val isSubmitting: Boolean = false,
    @StringRes val errorRes: Int? = null,
    val submitResult: SubmitResult? = null,
    val kind:String="bug",
    val category:String="other",
    val includeAssistant:Boolean=false,
    val includeAllChats:Boolean=false,
    val recordingEnabled:Boolean=false,
    val preview:PreparedFeedback?=null,
    val reportId:String?=null,
    val reports:List<com.lelloman.pezzottify.android.domain.remoteapi.FeedbackSummary> = emptyList(),
    val nextCursor:Long?=null,
    val reportsLoading:Boolean=false,
    val reportsError:Boolean=false,
    val retryAfterSeconds:Long?=null,
)

class PreparedFeedback(
    val request:com.lelloman.pezzottify.android.domain.remoteapi.FeedbackReport,
    val send:suspend ()->com.lelloman.pezzottify.android.domain.remoteapi.FeedbackResult,
)

sealed interface SubmitResult {
    data object Success : SubmitResult
    data class Error(val message: String) : SubmitResult
}
