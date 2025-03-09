package com.lelloman.pezzottify.android.ui.screen.main.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.content.ContentResolver
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SearchScreenViewModel @Inject constructor(
    private val interactor: Interactor,
    private val contentResolver: ContentResolver
) : ViewModel(),
    SearchScreenActions {

    private val mutableState = MutableStateFlow(SearchScreenState())
    val state = mutableState.asStateFlow()

    private var previousSearchJob: Job? = null

    override fun updateQuery(query: String) {
        mutableState.value = mutableState.value.copy(
            query = query,
            isLoading = true,
        )
        previousSearchJob?.cancel()
        previousSearchJob = viewModelScope.launch {
            val searchResultsResult = interactor.search(query)
            mutableState.value = mutableState.value.copy(
                isLoading = false,
                searchResults = searchResultsResult.getOrNull()
                    ?.map { contentResolver.resolveSearchResult(it) },
                searchError = searchResultsResult.exceptionOrNull()?.let { "Error" }
            )
        }
    }

    interface Interactor {
        suspend fun search(query: String): Result<List<String>>
    }
}