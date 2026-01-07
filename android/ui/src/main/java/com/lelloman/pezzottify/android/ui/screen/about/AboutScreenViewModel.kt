package com.lelloman.pezzottify.android.ui.screen.about

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class AboutScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(AboutScreenState())
    val state: StateFlow<AboutScreenState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            mutableState.update {
                it.copy(
                    versionName = interactor.getVersionName(),
                    gitCommit = interactor.getGitCommit(),
                    serverUrl = interactor.getServerUrl(),
                )
            }
        }
        viewModelScope.launch {
            interactor.observeServerVersion().collect { serverVersion ->
                mutableState.update { it.copy(serverVersion = serverVersion) }
            }
        }
    }

    interface Interactor {
        fun getVersionName(): String
        fun getGitCommit(): String
        fun getServerUrl(): String
        fun observeServerVersion(): Flow<String>
    }
}
