package com.lelloman.pezzottify.android.ui.screen.main

interface MainScreenActions {

    fun clickOnTab(tab: MainScreenState.Tab)

    suspend fun clickOnProfile()
}