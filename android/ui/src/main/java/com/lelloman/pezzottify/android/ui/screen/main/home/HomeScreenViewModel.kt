package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.component.ConnectionState
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import java.util.Calendar
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class HomeScreenViewModel(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver,
    private val coroutineContext: CoroutineContext,
) : HomeScreenActions, ViewModel() {

    @Inject
    constructor(
        interactor: Interactor,
        contentResolver: ContentResolver,
    ) : this(
        interactor,
        contentResolver,
        Dispatchers.IO,
    )

    private val mutableEvents = MutableSharedFlow<HomeScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    private val mutableState = MutableStateFlow(HomeScreenState())
    val state = mutableState.asStateFlow()

    init {
        viewModelScope.launch(coroutineContext) {
            // Load user name
            val userName = interactor.getUserName()
            mutableState.value = mutableState.value.copy(userName = userName)

            interactor.getRecentlyViewedContent(8)
                .map { it.map(::resolveRecentlyViewedContent) }
                .collect {
                    mutableState.value = mutableState.value.copy(recentlyViewedContent = it)
                }
        }

        // Fetch popular content
        viewModelScope.launch(coroutineContext) {
            val popularContent = interactor.getPopularContent()
            mutableState.value = mutableState.value.copy(popularContent = popularContent)
        }

        viewModelScope.launch {
            interactor.connectionState(viewModelScope).collect { connectionState ->
                mutableState.value = mutableState.value.copy(connectionState = connectionState)
            }
        }
    }

    override fun clickOnRecentlyViewedItem(itemId: String, itemType: ViewedContentType) {
        viewModelScope.launch {
            when (itemType) {
                ViewedContentType.Artist -> mutableEvents.emit(
                    HomeScreenEvents.NavigateToArtist(
                        itemId
                    )
                )

                ViewedContentType.Album -> mutableEvents.emit(
                    HomeScreenEvents.NavigateToAlbum(
                        itemId
                    )
                )

                ViewedContentType.Playlist -> TODO()
                ViewedContentType.Track -> mutableEvents.emit(
                    HomeScreenEvents.NavigateToTrack(
                        itemId
                    )
                )
            }
        }
    }

    private fun resolveRecentlyViewedContent(recentlyViewedContent: HomeScreenState.RecentlyViewedContent): Flow<Content<ResolvedRecentlyViewedContent>> =
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
                                contentState.data.artistsIds.map { ResolvedArtistInfo(it, "") },
                                contentState.data.date.toYear(),
                            )
                        )

                        else -> contentState as Content<ResolvedRecentlyViewedContent>
                    }
                }
                .resolveArtists()

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

    override suspend fun clickOnProfile() {
        mutableEvents.emit(HomeScreenEvents.NavigateToProfileScreen)
    }

    override suspend fun clickOnSettings() {
        mutableEvents.emit(HomeScreenEvents.NavigateToSettingsScreen)
    }

    override fun clickOnPopularAlbum(albumId: String) {
        viewModelScope.launch {
            mutableEvents.emit(HomeScreenEvents.NavigateToAlbum(albumId))
        }
    }

    override fun clickOnPopularArtist(artistId: String) {
        viewModelScope.launch {
            mutableEvents.emit(HomeScreenEvents.NavigateToArtist(artistId))
        }
    }

    interface Interactor {

        fun connectionState(scope: CoroutineScope): StateFlow<ConnectionState>

        suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<HomeScreenState.RecentlyViewedContent>>
        fun getUserName(): String
        suspend fun getPopularContent(): PopularContentState?
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    private fun Flow<Content<ResolvedRecentlyViewedContent>>.resolveArtists(): Flow<Content<ResolvedRecentlyViewedContent>> =
        flatMapLatest { content ->
            when (content) {
                is Content.Resolved -> {
                    val artistFlows = content.data.artists.map { artistInfo ->
                        contentResolver.resolveArtist(artistInfo.id).map { artistContent ->
                            when (artistContent) {
                                is Content.Resolved -> ResolvedArtistInfo(
                                    artistContent.data.id,
                                    artistContent.data.name
                                )

                                else -> artistInfo
                            }
                        }
                    }
                    if (artistFlows.isEmpty()) {
                        flowOf(content)
                    } else {
                        combine(artistFlows) { artists ->
                            Content.Resolved(
                                itemId = content.itemId,
                                data = content.data.copy(artists = artists.toList())
                            )
                        }
                    }
                }

                else -> flowOf(content)
            }
        }
}

private fun Long.toYear(): Int {
    val calendar = Calendar.getInstance()
    calendar.timeInMillis = this * 1000 // Convert seconds to milliseconds
    return calendar.get(Calendar.YEAR)
}