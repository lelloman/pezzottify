package com.lelloman.pezzottify.android.ui.screen.main

import androidx.lifecycle.ViewModel
import com.lelloman.pezzottify.android.ui.screen.main.home.HomeScreenEvents
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject

@HiltViewModel
class MainViewModel @Inject constructor() : ViewModel(), MainScreenActions {

    private val mutableState = MutableStateFlow(MainScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<HomeScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    override fun clickOnTab(tab: MainScreenState.Tab) {
        mutableState.value = mutableState.value.copy(tab = tab)
    }

    override suspend fun clickOnProfile() {
        mutableEvents.emit(HomeScreenEvents.NavigateToProfileScreen)
    }
}