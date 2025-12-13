package com.lelloman.pezzottify.android.ui.screen.main.listeninghistory

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class ListeningHistoryScreenViewModel(
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    private val coroutineContext: CoroutineContext,
) : ViewModel() {

    @Inject
    constructor(
        interactor: Interactor,
        contentResolver: ContentResolver,
    ) : this(
        interactor,
        contentResolver,
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow(ListeningHistoryScreenState())
    val state = mutableState.asStateFlow()

    private var currentOffset = 0
    private val pageSize = 50

    init {
        loadInitialData()
    }

    private fun loadInitialData() {
        currentOffset = 0
        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true, errorRes = null)

            val result = interactor.getListeningEvents(limit = pageSize, offset = 0)
            mutableState.value = if (result.isSuccess) {
                val events = result.getOrNull() ?: emptyList()
                currentOffset = events.size
                mutableState.value.copy(
                    isLoading = false,
                    events = events,
                    errorRes = null,
                    hasMorePages = events.size >= pageSize,
                )
            } else {
                val errorRes = getErrorStringRes(result.exceptionOrNull())
                mutableState.value.copy(
                    isLoading = false,
                    errorRes = errorRes,
                )
            }
        }
    }

    private fun getErrorStringRes(exception: Throwable?): Int {
        val errorType = (exception as? ListeningHistoryException)?.errorType
        return when (errorType) {
            ListeningHistoryErrorType.Network -> R.string.listening_history_error_network
            ListeningHistoryErrorType.Unauthorized -> R.string.listening_history_error_unauthorized
            else -> R.string.listening_history_error_unknown
        }
    }

    fun refresh() {
        loadInitialData()
    }

    fun loadMore() {
        if (mutableState.value.isLoading || !mutableState.value.hasMorePages) return

        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true)

            val result = interactor.getListeningEvents(limit = pageSize, offset = currentOffset)
            mutableState.value = if (result.isSuccess) {
                val newEvents = result.getOrNull() ?: emptyList()
                currentOffset += newEvents.size
                mutableState.value.copy(
                    isLoading = false,
                    events = mutableState.value.events + newEvents,
                    hasMorePages = newEvents.size >= pageSize,
                )
            } else {
                mutableState.value.copy(
                    isLoading = false,
                )
            }
        }
    }

    interface Interactor {
        suspend fun getListeningEvents(limit: Int, offset: Int): Result<List<UiListeningEvent>>
    }
}
