package com.lelloman.pezzottify.android.ui.screen.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.logger.LoggerFactory
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import com.lelloman.pezzottify.android.ui.content.Track
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.launch
import javax.inject.Inject

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class MainScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    loggerFactory: LoggerFactory,
    private val contentResolver: ContentResolver,
) : ViewModel(), MainScreenActions {

    private val logger by loggerFactory

    private val mutableState = MutableStateFlow(MainScreenState())
    val state = mutableState.asStateFlow()

    private val playingTrackId = MutableStateFlow<String?>(null)

    init {
        viewModelScope.launch {
            interactor.getPlaybackState()
                .collect {
                    logger.debug("Collecting new playback state $it")
                    val oldBottomPlayerState = mutableState.value.bottomPlayer
                    val newBottomPlayerState = when (it) {
                        is Interactor.PlaybackState.Idle, null -> MainScreenState.BottomPlayer()
                        is Interactor.PlaybackState.Loaded -> {
                            playingTrackId.value = it.trackId
                            if (oldBottomPlayerState.trackId != it.trackId) {
                                MainScreenState.BottomPlayer(
                                    isVisible = true,
                                    trackId = it.trackId,
                                    trackName = "",
                                    isPlaying = it.isPlaying,
                                )
                            } else {
                                oldBottomPlayerState.copy(
                                    isVisible = true,
                                    isPlaying = it.isPlaying,
                                )
                            }
                        }
                    }
                    mutableState.value =
                        mutableState.value.copy(bottomPlayer = newBottomPlayerState)
                }
        }

        viewModelScope.launch {
            playingTrackId.filterNotNull()
                .flatMapLatest {
                    logger.debug("BottomPlayer new track id")
                    contentResolver.resolveTrack(it)
                        .filterIsInstance<Content.Resolved<Track>>()
                        .take(1)
                }
                .onEach {
                    logger.debug("BottomPlayer resolved track name -> $it")
                    val oldState = mutableState.value
                    if (oldState.bottomPlayer.trackId == it.itemId) {
                        mutableState.value =
                            oldState.copy(bottomPlayer = oldState.bottomPlayer.copy(trackName = it.data.name))
                    }
                }
                .collect {

                }
        }
    }

    override fun clickOnPlayPause() = interactor.clickOnPlayPause()

    override fun clickOnSkipToNext() = interactor.clickOnSkipToNext()

    override fun clickOnSkipToPrevious() = interactor.clickOnSkipToPrevious()

    interface Interactor {

        fun getPlaybackState(): Flow<PlaybackState?>

        fun clickOnPlayPause()

        fun clickOnSkipToNext()

        fun clickOnSkipToPrevious()

        sealed interface PlaybackState {
            data object Idle : PlaybackState

            data class Loaded(
                val isPlaying: Boolean,
                val trackId: String,
                val trackPercent: Float,
            ) : PlaybackState
        }
    }
}