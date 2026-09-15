package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

interface BugReportScreenActions {
    fun onTitleChanged(title: String)
    fun onDescriptionChanged(description: String)
    fun onIncludeLogsChanged(includeLogs: Boolean)
    fun submit()
    fun onKindChanged(kind:String) {}
    fun onCategoryChanged(category:String) {}
    fun onIncludeAssistantChanged(include:Boolean) {}
    fun onIncludeAllChatsChanged(include:Boolean) {}
    fun onRecordingChanged(enabled:Boolean) {}
    fun refreshReports(loadMore:Boolean=false) {}
}
