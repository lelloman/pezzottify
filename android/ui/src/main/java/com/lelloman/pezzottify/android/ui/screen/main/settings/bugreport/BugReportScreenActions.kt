package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

interface BugReportScreenActions {
    fun onTitleChanged(title: String)
    fun onDescriptionChanged(description: String)
    fun onIncludeLogsChanged(includeLogs: Boolean)
    fun submit()
}
