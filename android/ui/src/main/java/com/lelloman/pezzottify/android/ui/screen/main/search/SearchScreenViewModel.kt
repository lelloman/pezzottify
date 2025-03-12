package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class SearchScreenViewModel internal constructor(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
    private val coroutineContext: CoroutineContext,
) : ViewModel(),
    SearchScreenActions {
    @Inject
    constructor(
        interactor: Interactor,
        contentResolver: ContentResolver
    ) : this(interactor, contentResolver, Dispatchers.IO)


    private val mutableState = MutableStateFlow(SearchScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<SearchScreensEvents>()
    val events = mutableEvents.asSharedFlow()

    private var previousSearchJob: Job? = null

    override fun updateQuery(query: String) {
        mutableState.value = mutableState.value.copy(
            query = query,
            isLoading = true,
        )
        previousSearchJob?.cancel()
        if (query.isNotEmpty()) {
            previousSearchJob = viewModelScope.launch {
                delay(400)
                if (!isActive) {
                    return@launch
                }
                val searchResultsResult = interactor.search(query)
                mutableState.value = mutableState.value.copy(
                    isLoading = false,
                    searchResults = searchResultsResult.getOrNull()
                        ?.map { contentResolver.resolveSearchResult(it.first, it.second) },
                    searchError = searchResultsResult.exceptionOrNull()?.let { "Error" }
                )
            }
        } else {
            mutableState.value = mutableState.value.copy(
                isLoading = false,
                searchResults = null,
                searchError = null
            )
        }
    }

    override fun clickOnArtistSearchResult(artistId: String) {
        viewModelScope.launch {
            mutableEvents.emit(SearchScreensEvents.NavigateToArtistScreen(artistId))
        }
        logViewed(artistId, SearchedItemType.Artist)
    }

    override fun clickOnAlbumSearchResult(albumId: String) {
        viewModelScope.launch {
            mutableEvents.emit(SearchScreensEvents.NavigateToAlbumScreen(albumId))
        }
        logViewed(albumId, SearchedItemType.Album)
    }

    override fun clickOnTrackSearchResult(trackId: String) {
        viewModelScope.launch {
            mutableEvents.emit(SearchScreensEvents.NavigateToTrackScreen(trackId))
        }
        logViewed(trackId, SearchedItemType.Track)
    }

    private fun logViewed(contentId: String, contentType: SearchedItemType) {
        viewModelScope.launch(coroutineContext) {
            interactor.logViewedContent(contentId, contentType)
        }
    }

    interface Interactor {
        suspend fun search(query: String): Result<List<Pair<String, SearchedItemType>>>

        suspend fun logViewedContent(contentId: String, contentType: SearchedItemType)
    }

    enum class SearchedItemType {
        Album,
        Track,
        Artist,
    }
}