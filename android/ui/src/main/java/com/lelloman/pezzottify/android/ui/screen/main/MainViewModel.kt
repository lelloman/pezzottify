package com.lelloman.pezzottify.android.ui.screen.main

import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import javax.inject.Inject

@HiltViewModel
class MainViewModel @Inject constructor() : ViewModel(), MainScreenActions {

    private val mutableState = MutableStateFlow(MainScreenState())
    val state = mutableState.asStateFlow()

    override fun clickOnTab(tab: MainScreenState.Tab) {
        mutableState.value = mutableState.value.copy(tab = tab)
    }
}