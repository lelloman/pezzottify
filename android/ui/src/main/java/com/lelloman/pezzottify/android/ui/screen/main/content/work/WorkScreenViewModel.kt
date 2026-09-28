package com.lelloman.pezzottify.android.ui.screen.main.content.work

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.remoteapi.response.TrackResponse
import com.lelloman.pezzottify.android.domain.statics.*
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWork
import com.lelloman.pezzottify.android.domain.statics.usecase.WorkNotFoundException
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.assisted.Assisted
import dagger.assisted.AssistedFactory
import dagger.assisted.AssistedInject
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class WorkScreenState(
    val work: Work? = null,
    val relations: List<WorkRelation> = emptyList(),
    val tracks: List<TrackResponse> = emptyList(),
    val scope: WorkScope = WorkScope.All,
    val loading: Boolean = false,
    val failed: Boolean = false,
    val notFound: Boolean = false,
    val hasMore: Boolean = false,
    val nextOffset: Int = 0,
)

@HiltViewModel(assistedFactory = WorkScreenViewModel.Factory::class)
class WorkScreenViewModel @AssistedInject constructor(
    private val getWork: GetWork,
    private val interactor: Interactor,
    val contentResolver: ContentResolver,
    @Assisted private val workId: String,
) : ViewModel() {
    private val mutableState = MutableStateFlow(WorkScreenState())
    val state = mutableState.asStateFlow()
    private var request: Job? = null
    private var generation = 0

    init { loadMore() }

    fun changeScope(scope: WorkScope) {
        if (scope == state.value.scope) return
        generation++
        request?.cancel()
        mutableState.update { it.copy(scope = scope, tracks = emptyList(), nextOffset = 0, hasMore = false, loading = false, failed = false) }
        loadMore()
    }

    fun loadMore() {
        if (state.value.loading) return
        val snapshot = state.value
        val token = generation
        mutableState.update { it.copy(loading = true, failed = false, notFound = false) }
        request = viewModelScope.launch {
            try {
                val page = getWork(workId, snapshot.nextOffset, snapshot.scope)
                ensureActive()
                if (token != generation) return@launch
                mutableState.update {
                    it.copy(work = page.work, relations = page.relations.sortedBy { relation -> relation.ordering },
                        tracks = (it.tracks + page.tracks).distinctBy { track -> track.track.id },
                        nextOffset = page.nextOffset, hasMore = page.hasMore, loading = false)
                }
            } catch (error: CancellationException) {
                throw error
            } catch (error: Exception) {
                if (token == generation) mutableState.update {
                    it.copy(loading = false, failed = true, notFound = error is WorkNotFoundException)
                }
            }
        }
    }

    fun play(track: TrackResponse) {
        if (track.track.availability == TrackAvailability.Available) interactor.playTrack(track.track.id)
    }

    interface Interactor { fun playTrack(trackId: String) }

    @AssistedFactory
    interface Factory { fun create(workId: String): WorkScreenViewModel }
}
