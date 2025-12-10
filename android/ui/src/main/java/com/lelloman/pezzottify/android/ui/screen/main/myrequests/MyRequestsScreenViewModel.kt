package com.lelloman.pezzottify.android.ui.screen.main.myrequests

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class MyRequestsScreenViewModel(
    private val interactor: Interactor,
    private val coroutineContext: CoroutineContext,
) : ViewModel(),
    MyRequestsScreenActions {

    @Inject
    constructor(
        interactor: Interactor,
    ) : this(
        interactor,
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow(MyRequestsScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<MyRequestsScreenEvent>()
    val events = mutableEvents.asSharedFlow()

    init {
        loadData()
    }

    private fun loadData() {
        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true, error = null)

            // Load limits
            val limitsResult = interactor.getDownloadLimits()
            limitsResult.getOrNull()?.let { limits ->
                mutableState.value = mutableState.value.copy(
                    limits = UiRequestLimits(
                        requestsToday = limits.requestsToday,
                        maxPerDay = limits.maxPerDay,
                        inQueue = limits.inQueue,
                        maxQueue = limits.maxQueue,
                    )
                )
            }

            // Load requests
            val requestsResult = interactor.getMyRequests()
            mutableState.value = if (requestsResult.isSuccess) {
                mutableState.value.copy(
                    isLoading = false,
                    requests = requestsResult.getOrNull(),
                    error = null,
                )
            } else {
                mutableState.value.copy(
                    isLoading = false,
                    error = "Failed to load requests",
                )
            }
        }
    }

    override fun refresh() {
        loadData()
    }

    override fun onRequestClick(request: UiDownloadRequest) {
        // If the request is completed and has a catalog ID, navigate to the album
        if (request.status == RequestStatus.Completed && request.catalogId != null) {
            viewModelScope.launch {
                mutableEvents.emit(MyRequestsScreenEvent.NavigateToAlbum(request.catalogId))
            }
        }
    }

    interface Interactor {
        suspend fun getMyRequests(): Result<List<UiDownloadRequest>>
        suspend fun getDownloadLimits(): Result<DownloadLimitsData>
    }

    data class DownloadLimitsData(
        val requestsToday: Int,
        val maxPerDay: Int,
        val inQueue: Int,
        val maxQueue: Int,
    )
}

sealed class MyRequestsScreenEvent {
    data class NavigateToAlbum(val albumId: String) : MyRequestsScreenEvent()
}
