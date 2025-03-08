package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import javax.inject.Inject

@HiltViewModel
class HomeScreenViewModel @Inject constructor() : HomeScreenActions, ViewModel() {

    private val mutableEvents = MutableSharedFlow<HomeScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    override suspend fun clickOnProfile() {
        mutableEvents.emit(HomeScreenEvents.NavigateToProfileScreen)
    }
}