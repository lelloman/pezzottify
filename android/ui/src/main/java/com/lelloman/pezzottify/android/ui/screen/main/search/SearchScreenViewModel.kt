package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.screen.main.home.ResolvedRecentlyViewedContent
import com.lelloman.pezzottify.android.ui.screen.main.home.ViewedContentType
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class SearchScreenViewModel(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
    private val coroutineContext: CoroutineContext,
) : ViewModel(),
    SearchScreenActions {

    @Inject
    constructor(
        interactor: Interactor,
        contentResolver: ContentResolver,
    ) : this(
        interactor,
        contentResolver,
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow(SearchScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<SearchScreensEvents>()
    val events = mutableEvents.asSharedFlow()

    private var previousSearchJob: Job? = null
    private var currentQuery: String = ""

    init {
        viewModelScope.launch(coroutineContext) {
            interactor.getRecentlyViewedContent(MAX_RECENTLY_VIEWED_ITEMS)
                .map { it.map(::resolveRecentlyViewedContent) }
                .collect {
                    mutableState.value = mutableState.value.copy(recentlyViewedContent = it)
                }
        }
        viewModelScope.launch(coroutineContext) {
            interactor.getSearchHistoryEntries(MAX_SEARCH_HISTORY_ITEMS)
                .map { it.map(::resolveSearchHistoryEntry) }
                .collect {
                    mutableState.value = mutableState.value.copy(searchHistoryItems = it)
                }
        }
    }

    override fun updateQuery(query: String) {
        currentQuery = query
        mutableState.value = mutableState.value.copy(
            query = query,
            isLoading = true,
        )
        performSearch()
    }

    override fun toggleFilter(filter: SearchFilter) {
        val currentFilters = mutableState.value.selectedFilters
        val newFilters = if (currentFilters.contains(filter)) {
            currentFilters - filter
        } else {
            currentFilters + filter
        }
        mutableState.value = mutableState.value.copy(selectedFilters = newFilters)
        // Re-run search with new filters if there's a query
        if (currentQuery.isNotEmpty()) {
            performSearch()
        }
    }

    private fun performSearch() {
        previousSearchJob?.cancel()
        if (currentQuery.isNotEmpty()) {
            previousSearchJob = viewModelScope.launch {
                delay(400)
                if (!isActive) {
                    return@launch
                }
                val filters = mutableState.value.selectedFilters.map { it.toInteractorFilter() }
                val searchResultsResult = interactor.search(
                    query = currentQuery,
                    filters = filters.ifEmpty { null }
                )
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

    private fun SearchFilter.toInteractorFilter(): InteractorSearchFilter = when (this) {
        SearchFilter.Album -> InteractorSearchFilter.Album
        SearchFilter.Artist -> InteractorSearchFilter.Artist
        SearchFilter.Track -> InteractorSearchFilter.Track
    }

    override fun clickOnArtistSearchResult(artistId: String) {
        if (currentQuery.isNotEmpty()) {
            interactor.logSearchHistoryEntry(currentQuery, SearchHistoryEntryType.Artist, artistId)
        }
        viewModelScope.launch {
            mutableEvents.emit(SearchScreensEvents.NavigateToArtistScreen(artistId))
        }
    }

    override fun clickOnAlbumSearchResult(albumId: String) {
        if (currentQuery.isNotEmpty()) {
            interactor.logSearchHistoryEntry(currentQuery, SearchHistoryEntryType.Album, albumId)
        }
        viewModelScope.launch {
            mutableEvents.emit(SearchScreensEvents.NavigateToAlbumScreen(albumId))
        }
    }

    override fun clickOnTrackSearchResult(trackId: String) {
        if (currentQuery.isNotEmpty()) {
            interactor.logSearchHistoryEntry(currentQuery, SearchHistoryEntryType.Track, trackId)
        }
        viewModelScope.launch {
            mutableEvents.emit(SearchScreensEvents.NavigateToTrackScreen(trackId))
        }
    }

    override fun clickOnRecentlyViewedItem(itemId: String, itemType: ViewedContentType) {
        viewModelScope.launch {
            when (itemType) {
                ViewedContentType.Artist -> mutableEvents.emit(SearchScreensEvents.NavigateToArtistScreen(itemId))
                ViewedContentType.Album -> mutableEvents.emit(SearchScreensEvents.NavigateToAlbumScreen(itemId))
                ViewedContentType.Track -> mutableEvents.emit(SearchScreensEvents.NavigateToTrackScreen(itemId))
                ViewedContentType.Playlist -> Unit
            }
        }
    }

    override fun clickOnSearchHistoryItem(itemId: String, itemType: ViewedContentType) {
        viewModelScope.launch {
            when (itemType) {
                ViewedContentType.Artist -> mutableEvents.emit(SearchScreensEvents.NavigateToArtistScreen(itemId))
                ViewedContentType.Album -> mutableEvents.emit(SearchScreensEvents.NavigateToAlbumScreen(itemId))
                ViewedContentType.Track -> mutableEvents.emit(SearchScreensEvents.NavigateToTrackScreen(itemId))
                ViewedContentType.Playlist -> Unit
            }
        }
    }

    private fun resolveRecentlyViewedContent(recentlyViewedContent: RecentlyViewedContent): Flow<Content<ResolvedRecentlyViewedContent>> =
        when (recentlyViewedContent.contentType) {
            ViewedContentType.Artist -> contentResolver.resolveArtist(recentlyViewedContent.contentId)
                .map { contentState ->
                    when (contentState) {
                        is Content.Resolved -> Content.Resolved(
                            itemId = contentState.data.id,
                            data = ResolvedRecentlyViewedContent(
                                contentState.data.id,
                                contentState.data.name,
                                contentState.data.imageUrl,
                                ViewedContentType.Artist,
                            )
                        )

                        else -> contentState as Content<ResolvedRecentlyViewedContent>
                    }
                }

            ViewedContentType.Album -> contentResolver.resolveAlbum(recentlyViewedContent.contentId)
                .map { contentState ->
                    when (contentState) {
                        is Content.Resolved -> Content.Resolved(
                            itemId = contentState.data.id,
                            data = ResolvedRecentlyViewedContent(
                                contentState.data.id,
                                contentState.data.name,
                                contentState.data.imageUrl,
                                ViewedContentType.Album,
                            )
                        )

                        else -> contentState as Content<ResolvedRecentlyViewedContent>
                    }
                }

            ViewedContentType.Track -> contentResolver.resolveTrack(recentlyViewedContent.contentId)
                .map { contentState ->
                    when (contentState) {
                        is Content.Resolved -> Content.Resolved(
                            itemId = contentState.data.id,
                            data = ResolvedRecentlyViewedContent(
                                contentState.data.id,
                                contentState.data.name,
                                null,
                                ViewedContentType.Track,
                            )
                        )

                        else -> contentState as Content<ResolvedRecentlyViewedContent>
                    }
                }

            ViewedContentType.Playlist -> flow {}
        }

    private fun resolveSearchHistoryEntry(entry: SearchHistoryEntry): Flow<Content<SearchHistoryItem>> =
        when (entry.contentType) {
            ViewedContentType.Artist -> contentResolver.resolveArtist(entry.contentId)
                .map { contentState ->
                    when (contentState) {
                        is Content.Resolved -> Content.Resolved(
                            itemId = contentState.data.id,
                            data = SearchHistoryItem(
                                query = entry.query,
                                contentId = contentState.data.id,
                                contentName = contentState.data.name,
                                contentImageUrl = contentState.data.imageUrl,
                                contentType = ViewedContentType.Artist,
                            )
                        )
                        else -> contentState as Content<SearchHistoryItem>
                    }
                }

            ViewedContentType.Album -> contentResolver.resolveAlbum(entry.contentId)
                .map { contentState ->
                    when (contentState) {
                        is Content.Resolved -> Content.Resolved(
                            itemId = contentState.data.id,
                            data = SearchHistoryItem(
                                query = entry.query,
                                contentId = contentState.data.id,
                                contentName = contentState.data.name,
                                contentImageUrl = contentState.data.imageUrl,
                                contentType = ViewedContentType.Album,
                            )
                        )
                        else -> contentState as Content<SearchHistoryItem>
                    }
                }

            ViewedContentType.Track -> contentResolver.resolveTrack(entry.contentId)
                .map { contentState ->
                    when (contentState) {
                        is Content.Resolved -> Content.Resolved(
                            itemId = contentState.data.id,
                            data = SearchHistoryItem(
                                query = entry.query,
                                contentId = contentState.data.id,
                                contentName = contentState.data.name,
                                contentImageUrl = null,
                                contentType = ViewedContentType.Track,
                            )
                        )
                        else -> contentState as Content<SearchHistoryItem>
                    }
                }

            ViewedContentType.Playlist -> flow {}
        }

    data class RecentlyViewedContent(
        val contentId: String,
        val contentType: ViewedContentType,
    )

    data class SearchHistoryEntry(
        val query: String,
        val contentId: String,
        val contentType: ViewedContentType,
    )

    interface Interactor {
        suspend fun search(
            query: String,
            filters: List<InteractorSearchFilter>? = null
        ): Result<List<Pair<String, SearchedItemType>>>
        suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<RecentlyViewedContent>>
        fun getSearchHistoryEntries(maxCount: Int): Flow<List<SearchHistoryEntry>>
        fun logSearchHistoryEntry(query: String, contentType: SearchHistoryEntryType, contentId: String)
    }

    enum class InteractorSearchFilter {
        Album,
        Artist,
        Track,
    }

    enum class SearchedItemType {
        Album,
        Track,
        Artist,
    }

    enum class SearchHistoryEntryType {
        Album,
        Track,
        Artist,
    }

    companion object {
        private const val MAX_RECENTLY_VIEWED_ITEMS = 10
        private const val MAX_SEARCH_HISTORY_ITEMS = 10
    }
}