package com.lelloman.pezzottify.android.ui.screen.main.settings.logviewer

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject

@HiltViewModel
class LogViewerScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(LogViewerScreenState())
    val state: StateFlow<LogViewerScreenState> = mutableState.asStateFlow()

    init {
        loadLogs()
    }

    private fun loadLogs() {
        viewModelScope.launch {
            val content = withContext(Dispatchers.IO) {
                interactor.getLogContent()
            }
            mutableState.update {
                it.copy(
                    logContent = content,
                    isLoading = false,
                    isRefreshing = false,
                )
            }
        }
    }

    fun onSearchQueryChanged(query: String) {
        mutableState.update { it.copy(searchQuery = query) }
    }

    fun onRefresh() {
        mutableState.update { it.copy(isRefreshing = true) }
        loadLogs()
    }

    fun toggleLevel(level: LogLevel) {
        mutableState.update { state ->
            val newLevels = if (level in state.enabledLevels) {
                state.enabledLevels - level
            } else {
                state.enabledLevels + level
            }
            state.copy(enabledLevels = newLevels)
        }
    }

    interface Interactor {
        fun getLogContent(): String
    }
}
