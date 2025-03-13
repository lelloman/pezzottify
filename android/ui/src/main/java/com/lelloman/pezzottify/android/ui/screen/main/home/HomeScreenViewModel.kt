package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
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
            interactor.getRecentlyViewedContent(8)
                .map { it.map(::resolveRecentlyViewedContent) }
                .collect {
                    mutableState.value = mutableState.value.copy(recentlyViewedContent = it)
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
                                "",
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
                                "",
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
                                "",
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

    interface Interactor {
        suspend fun getRecentlyViewedContent(maxCount: Int): Flow<List<HomeScreenState.RecentlyViewedContent>>
    }
}