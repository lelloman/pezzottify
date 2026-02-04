package com.lelloman.pezzottify.android.ui.screen.main.myrequests

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
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
    val contentResolver: ContentResolver,
    private val coroutineContext: CoroutineContext,
) : ViewModel(),
    MyRequestsScreenActions {

    @Inject
    constructor(
        interactor: Interactor,
        contentResolver: ContentResolver,
    ) : this(
        interactor,
        contentResolver,
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow(MyRequestsScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<MyRequestsScreenEvent>()
    val events = mutableEvents.asSharedFlow()

    init {
        loadData()
        subscribeToUpdates()
    }

    private fun subscribeToUpdates() {
        viewModelScope.launch(coroutineContext) {
            interactor.observeUpdates().collect { update ->
                applyUpdate(update)
            }
        }
    }

    private fun applyUpdate(update: UiDownloadStatusUpdate) {
        val currentRequests = mutableState.value.requests ?: return
        val updatedRequests = when (update) {
            is UiDownloadStatusUpdate.ProgressUpdated -> {
                currentRequests.map { request ->
                    if (request.id == update.requestId) {
                        request.copy(
                            progress = RequestProgress(
                                current = update.completed,
                                total = update.total,
                            ),
                            status = RequestStatus.InProgress,
                        )
                    } else request
                }
            }
            is UiDownloadStatusUpdate.StatusChanged -> {
                currentRequests.map { request ->
                    if (request.id == update.requestId) {
                        request.copy(
                            status = update.status,
                            queuePosition = update.queuePosition,
                            errorMessage = update.errorMessage,
                        )
                    } else request
                }
            }
            is UiDownloadStatusUpdate.Completed -> {
                currentRequests.map { request ->
                    if (request.id == update.requestId) {
                        request.copy(
                            status = RequestStatus.Completed,
                            completedAt = System.currentTimeMillis(),
                        )
                    } else request
                }
            }
            is UiDownloadStatusUpdate.Created -> {
                // New request - add to list
                currentRequests + UiDownloadRequest(
                    id = update.requestId,
                    albumName = update.contentName,
                    artistName = update.artistName ?: "",
                    status = RequestStatus.Pending,
                    queuePosition = update.queuePosition,
                    catalogId = update.contentId,
                    createdAt = System.currentTimeMillis(),
                )
            }
        }
        mutableState.value = mutableState.value.copy(requests = updatedRequests)
    }

    private fun loadData() {
        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true, errorRes = null)

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

            // Load requests (first page)
            val requestsResult = interactor.getMyRequests(PAGE_SIZE, 0)
            mutableState.value = if (requestsResult.isSuccess) {
                val requests = requestsResult.getOrNull() ?: emptyList()
                mutableState.value.copy(
                    isLoading = false,
                    requests = requests,
                    errorRes = null,
                    hasMoreCompleted = requests.count { it.status == RequestStatus.Completed } >= PAGE_SIZE,
                )
            } else {
                mutableState.value.copy(
                    isLoading = false,
                    errorRes = R.string.failed_to_load_requests,
                )
            }
        }
    }

    fun loadMoreCompleted() {
        val currentRequests = mutableState.value.requests ?: return
        if (mutableState.value.isLoadingMore || !mutableState.value.hasMoreCompleted) return

        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoadingMore = true)

            val currentOffset = currentRequests.size
            val requestsResult = interactor.getMyRequests(PAGE_SIZE, currentOffset)

            if (requestsResult.isSuccess) {
                val newRequests = requestsResult.getOrNull() ?: emptyList()
                mutableState.value = mutableState.value.copy(
                    isLoadingMore = false,
                    requests = currentRequests + newRequests,
                    hasMoreCompleted = newRequests.count { it.status == RequestStatus.Completed } >= PAGE_SIZE,
                )
            } else {
                mutableState.value = mutableState.value.copy(isLoadingMore = false)
            }
        }
    }

    override fun refresh() {
        loadData()
    }

    override fun onRequestClick(request: UiDownloadRequest) {
        val albumId = request.catalogId ?: return
        viewModelScope.launch {
            mutableEvents.emit(MyRequestsScreenEvent.NavigateToAlbum(albumId))
        }
    }

    override fun onTabSelected(tab: MyRequestsTab) {
        mutableState.value = mutableState.value.copy(selectedTab = tab)
    }

    interface Interactor {
        suspend fun getMyRequests(limit: Int, offset: Int): Result<List<UiDownloadRequest>>
        suspend fun getDownloadLimits(): Result<DownloadLimitsData>
        fun observeUpdates(): Flow<UiDownloadStatusUpdate>
    }

    companion object {
        const val PAGE_SIZE = 50
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
