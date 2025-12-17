package com.lelloman.pezzottify.android.ui.screen.main.whatsnew

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class WhatsNewScreenViewModel(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
    private val coroutineContext: CoroutineContext,
) : ViewModel(), WhatsNewScreenActions {

    @Inject
    constructor(
        interactor: Interactor,
        contentResolver: ContentResolver,
    ) : this(
        interactor,
        contentResolver,
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow(WhatsNewScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<WhatsNewScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    init {
        loadBatches()
    }

    private fun loadBatches() {
        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true, error = null)

            val result = interactor.getWhatsNew(BATCH_LIMIT)
            result.fold(
                onSuccess = { batches ->
                    val uiBatches = batches.map { batch ->
                        UiBatch(
                            id = batch.batchId,
                            name = batch.batchName,
                            description = batch.description,
                            closedAt = batch.closedAt,
                            summary = UiBatchSummary(
                                artistsAdded = batch.artistsAdded,
                                albumsAdded = batch.albumsAdded,
                                tracksAdded = batch.tracksAdded,
                                artistsUpdated = batch.artistsUpdated,
                                albumsUpdated = batch.albumsUpdated,
                                tracksUpdated = batch.tracksUpdated,
                            ),
                            isExpanded = false,
                            albums = if (batch.albumIds.isNotEmpty()) {
                                batch.albumIds.take(MAX_ALBUMS_PER_BATCH).map { albumId ->
                                    contentResolver.resolveAlbum(albumId).map { contentState ->
                                        when (contentState) {
                                            is Content.Resolved -> Content.Resolved(
                                                itemId = contentState.data.id,
                                                data = UiWhatsNewAlbum(
                                                    id = contentState.data.id,
                                                    name = contentState.data.name,
                                                    imageUrl = contentState.data.imageUrl,
                                                )
                                            )
                                            else -> contentState as Content<UiWhatsNewAlbum>
                                        }
                                    }
                                }
                            } else null,
                        )
                    }
                    mutableState.value = mutableState.value.copy(
                        batches = uiBatches,
                        isLoading = false,
                    )
                },
                onFailure = { error ->
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        error = error.message ?: "Unknown error",
                    )
                }
            )
        }
    }

    override fun clickOnAlbum(albumId: String) {
        viewModelScope.launch {
            mutableEvents.emit(WhatsNewScreenEvents.NavigateToAlbum(albumId))
        }
    }

    override fun toggleBatchExpanded(batchId: String) {
        mutableState.value = mutableState.value.copy(
            batches = mutableState.value.batches.map { batch ->
                if (batch.id == batchId) {
                    batch.copy(isExpanded = !batch.isExpanded)
                } else {
                    batch
                }
            }
        )
    }

    override fun goBack() {
        viewModelScope.launch {
            mutableEvents.emit(WhatsNewScreenEvents.NavigateBack)
        }
    }

    interface Interactor {
        suspend fun getWhatsNew(limit: Int): Result<List<WhatsNewBatchData>>
    }

    data class WhatsNewBatchData(
        val batchId: String,
        val batchName: String,
        val description: String?,
        val closedAt: Long,
        val artistsAdded: Int,
        val albumsAdded: Int,
        val tracksAdded: Int,
        val artistsUpdated: Int,
        val albumsUpdated: Int,
        val tracksUpdated: Int,
        val albumIds: List<String>,
    )

    companion object {
        private const val BATCH_LIMIT = 20
        private const val MAX_ALBUMS_PER_BATCH = 20
    }
}
