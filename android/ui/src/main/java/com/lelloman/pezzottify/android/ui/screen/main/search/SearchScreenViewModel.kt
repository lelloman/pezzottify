package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
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
        viewModelScope.launch(coroutineContext) {
            interactor.canUseExternalSearch().collect { canUse ->
                mutableState.value = mutableState.value.copy(canUseExternalSearch = canUse)
            }
        }
        viewModelScope.launch(coroutineContext) {
            interactor.isExternalModeEnabled().collect { isEnabled ->
                mutableState.value = mutableState.value.copy(isExternalMode = isEnabled)
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

    override fun toggleExternalMode() {
        val newMode = !mutableState.value.isExternalMode
        viewModelScope.launch(coroutineContext) {
            interactor.setExternalModeEnabled(newMode)
        }
        // Clear current results when switching modes
        mutableState.value = mutableState.value.copy(
            isExternalMode = newMode,
            searchResults = null,
            searchErrorRes = null,
            externalResults = null,
            externalSearchErrorRes = null,
            selectedFilters = emptySet(),
        )
        // Re-run search if there's a query
        if (currentQuery.isNotEmpty()) {
            performSearch()
        }
    }

    override fun clickOnExternalResult(result: ExternalSearchResultContent) {
        viewModelScope.launch {
            // If the item is in catalog, navigate to catalog screen
            val catalogId = result.catalogId
            if (result.inCatalog && catalogId != null) {
                when (result) {
                    is ExternalSearchResultContent.Album ->
                        mutableEvents.emit(SearchScreensEvents.NavigateToAlbumScreen(catalogId))
                    is ExternalSearchResultContent.Artist ->
                        mutableEvents.emit(SearchScreensEvents.NavigateToArtistScreen(catalogId))
                }
            } else {
                // Navigate to external content screen
                when (result) {
                    is ExternalSearchResultContent.Album ->
                        mutableEvents.emit(SearchScreensEvents.NavigateToExternalAlbumScreen(result.id))
                    is ExternalSearchResultContent.Artist -> {
                        // TODO: Implement ExternalArtistScreen to show artist's discography
                        // For now, external artists not in catalog cannot be navigated to
                        // (only albums can be requested for download)
                    }
                }
            }
        }
    }

    override fun requestAlbumDownload(result: ExternalSearchResultContent.Album) {
        viewModelScope.launch(coroutineContext) {
            // Add to requesting set (shows loading state)
            mutableState.value = mutableState.value.copy(
                requestingAlbumIds = mutableState.value.requestingAlbumIds + result.id
            )

            val requestResult = interactor.requestAlbumDownload(
                albumId = result.id,
                albumName = result.name,
                artistName = result.artistName,
            )

            // Remove from requesting set
            mutableState.value = mutableState.value.copy(
                requestingAlbumIds = mutableState.value.requestingAlbumIds - result.id
            )

            if (requestResult.isSuccess) {
                // Update the result to show it's now in queue
                val currentExternalResults = mutableState.value.externalResults
                val updatedResults = currentExternalResults?.map { existing ->
                    if (existing is ExternalSearchResultContent.Album && existing.id == result.id) {
                        existing.copy(inQueue = true)
                    } else {
                        existing
                    }
                }
                mutableState.value = mutableState.value.copy(externalResults = updatedResults)
                // Refresh limits
                refreshLimits()
                // Show success feedback
                mutableEvents.emit(SearchScreensEvents.ShowRequestSuccess)
            } else {
                // Show error feedback
                mutableEvents.emit(SearchScreensEvents.ShowRequestError(R.string.failed_to_request_download))
            }
        }
    }

    private fun refreshLimits() {
        viewModelScope.launch(coroutineContext) {
            val limitsResult = interactor.getDownloadLimits()
            limitsResult.getOrNull()?.let { limits ->
                mutableState.value = mutableState.value.copy(
                    downloadLimits = UiDownloadLimits(
                        requestsToday = limits.requestsToday,
                        maxPerDay = limits.maxPerDay,
                        canRequest = limits.canRequest,
                        inQueue = limits.inQueue,
                        maxQueue = limits.maxQueue,
                    )
                )
            }
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

                // Only do external search if both external mode is enabled AND user can use it
                if (mutableState.value.isExternalMode && mutableState.value.canUseExternalSearch) {
                    performExternalSearch()
                } else {
                    performCatalogSearch()
                }
            }
        } else {
            mutableState.value = mutableState.value.copy(
                isLoading = false,
                searchResults = null,
                searchErrorRes = null,
                externalResults = null,
                externalSearchErrorRes = null,
            )
        }
    }

    private suspend fun performCatalogSearch() {
        val filters = mutableState.value.selectedFilters.map { it.toInteractorFilter() }
        val searchResultsResult = interactor.search(
            query = currentQuery,
            filters = filters.ifEmpty { null }
        )
        mutableState.value = mutableState.value.copy(
            isLoading = false,
            searchResults = searchResultsResult.getOrNull()
                ?.map { contentResolver.resolveSearchResult(it.first, it.second) },
            searchErrorRes = searchResultsResult.exceptionOrNull()?.let { R.string.error }
        )
    }

    private suspend fun performExternalSearch() {
        mutableState.value = mutableState.value.copy(
            externalSearchLoading = true,
            isLoading = true,
        )

        // Fetch limits in parallel with search
        val limitsDeferred = viewModelScope.launch(coroutineContext) {
            val limitsResult = interactor.getDownloadLimits()
            limitsResult.getOrNull()?.let { limits ->
                mutableState.value = mutableState.value.copy(
                    downloadLimits = UiDownloadLimits(
                        requestsToday = limits.requestsToday,
                        maxPerDay = limits.maxPerDay,
                        canRequest = limits.canRequest,
                        inQueue = limits.inQueue,
                        maxQueue = limits.maxQueue,
                    )
                )
            }
        }

        // Get selected filters or default to Album
        val selectedFilters = mutableState.value.selectedFilters
        val searchTypes = if (selectedFilters.isEmpty()) {
            listOf(InteractorExternalSearchType.Album)
        } else {
            selectedFilters.mapNotNull { filter ->
                when (filter) {
                    SearchFilter.Album -> InteractorExternalSearchType.Album
                    SearchFilter.Artist -> InteractorExternalSearchType.Artist
                    SearchFilter.Track -> null // External search doesn't support tracks
                }
            }.ifEmpty { listOf(InteractorExternalSearchType.Album) }
        }

        // Perform searches for each type and merge results
        val allResults = mutableListOf<ExternalSearchResultContent>()
        var hasError = false

        for (searchType in searchTypes) {
            val searchResult = interactor.externalSearch(currentQuery, searchType)
            if (searchResult.isSuccess) {
                searchResult.getOrNull()?.forEach { result ->
                    val content = when (searchType) {
                        InteractorExternalSearchType.Album -> ExternalSearchResultContent.Album(
                            id = result.id,
                            name = result.name,
                            artistName = result.artistName ?: "",
                            year = result.year,
                            imageUrl = result.imageUrl,
                            inCatalog = result.inCatalog,
                            inQueue = result.inQueue,
                            catalogId = result.catalogId,
                            score = result.score,
                        )
                        InteractorExternalSearchType.Artist -> ExternalSearchResultContent.Artist(
                            id = result.id,
                            name = result.name,
                            imageUrl = result.imageUrl,
                            inCatalog = result.inCatalog,
                            inQueue = result.inQueue,
                            catalogId = result.catalogId,
                            score = result.score,
                        )
                    }
                    allResults.add(content)
                }
            } else {
                hasError = true
            }
        }

        // Sort combined results by score (descending) for relevance ordering
        val sortedResults = allResults.sortedByDescending { it.score }

        limitsDeferred.join()

        mutableState.value = mutableState.value.copy(
            isLoading = false,
            externalSearchLoading = false,
            externalResults = sortedResults.ifEmpty { null },
            externalSearchErrorRes = if (hasError && sortedResults.isEmpty()) R.string.search_failed else null,
        )
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
        fun canUseExternalSearch(): Flow<Boolean>
        fun isExternalModeEnabled(): Flow<Boolean>
        suspend fun setExternalModeEnabled(enabled: Boolean)
        suspend fun externalSearch(query: String, type: InteractorExternalSearchType): Result<List<ExternalSearchItem>>
        suspend fun getDownloadLimits(): Result<DownloadLimitsData>
        suspend fun requestAlbumDownload(albumId: String, albumName: String, artistName: String): Result<Unit>
    }

    data class ExternalSearchItem(
        val id: String,
        val name: String,
        val artistName: String?,
        val year: Int?,
        val imageUrl: String?,
        val inCatalog: Boolean,
        val inQueue: Boolean,
        val catalogId: String?,
        val score: Float,
    )

    data class DownloadLimitsData(
        val requestsToday: Int,
        val maxPerDay: Int,
        val canRequest: Boolean,
        val inQueue: Int,
        val maxQueue: Int,
    )

    enum class InteractorExternalSearchType {
        Album,
        Artist,
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