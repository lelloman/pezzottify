package com.lelloman.pezzottify.android.ui.screen.main.myrequests

interface MyRequestsScreenActions {
    fun refresh()
    fun onRequestClick(request: UiDownloadRequest)
    fun onTabSelected(tab: MyRequestsTab)
}
