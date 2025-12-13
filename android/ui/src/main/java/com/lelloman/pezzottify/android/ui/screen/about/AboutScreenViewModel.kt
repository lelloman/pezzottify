package com.lelloman.pezzottify.android.ui.screen.about

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class AboutScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    private val mutableState = MutableStateFlow(AboutScreenState())
    val state: StateFlow<AboutScreenState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            val counts = interactor.getSkeletonCounts()
            mutableState.value = AboutScreenState(
                versionName = interactor.getVersionName(),
                gitCommit = interactor.getGitCommit(),
                serverUrl = interactor.getServerUrl(),
                artistCount = counts.artists,
                albumCount = counts.albums,
                trackCount = counts.tracks,
            )
        }
    }

    interface Interactor {
        fun getVersionName(): String
        fun getGitCommit(): String
        fun getServerUrl(): String
        suspend fun getSkeletonCounts(): SkeletonCountsData
    }

    data class SkeletonCountsData(
        val artists: Int,
        val albums: Int,
        val tracks: Int,
    )
}
