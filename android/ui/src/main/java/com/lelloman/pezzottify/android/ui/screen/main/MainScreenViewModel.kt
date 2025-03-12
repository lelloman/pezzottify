package com.lelloman.pezzottify.android.ui.screen.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.logger.LoggerFactory
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class MainScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    loggerFactory: LoggerFactory,
) : ViewModel(), MainScreenActions {

    private val logger by loggerFactory

    private val mutableState = MutableStateFlow(MainScreenState())
    val state = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            interactor.getPlaybackState().collect {
                logger.debug("Collecting new playback state $it")
                mutableState.value = mutableState.value.copy(
                    bottomPlayerVisible = it != null,
                    playerTrackName = it?.trackId.orEmpty(),
                    playerIsPlaying = it?.isPlaying == true,
                )
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
    }

    data class PlaybackState(
        val isPlaying: Boolean,
        val trackId: String,
        val trackPercent: Float,
    )
}