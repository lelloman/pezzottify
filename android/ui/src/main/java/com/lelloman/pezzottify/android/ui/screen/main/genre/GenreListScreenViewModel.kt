package com.lelloman.pezzottify.android.ui.screen.main.genre

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import javax.inject.Inject
import kotlin.coroutines.CoroutineContext

@HiltViewModel
class GenreListScreenViewModel(
    private val interactor: Interactor,
    private val coroutineContext: CoroutineContext,
) : ViewModel(), GenreListScreenActions {

    @Inject
    constructor(
        interactor: Interactor,
    ) : this(
        interactor,
        Dispatchers.IO,
    )

    private val mutableState = MutableStateFlow(GenreListScreenState())
    val state = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<GenreListScreenEvents>()
    val events = mutableEvents.asSharedFlow()

    init {
        loadGenres()
    }

    private fun loadGenres() {
        viewModelScope.launch(coroutineContext) {
            mutableState.value = mutableState.value.copy(isLoading = true, error = null)

            val result = interactor.getGenres()
            result.fold(
                onSuccess = { genres ->
                    val uiGenres = genres.map { genre ->
                        GenreListItemState(
                            name = genre.name,
                            trackCount = genre.trackCount,
                        )
                    }.sortedByDescending { it.trackCount }
                    mutableState.value = mutableState.value.copy(
                        genres = uiGenres,
                        filteredGenres = uiGenres,
                        isLoading = false,
                    )
                },
                onFailure = { error ->
                    mutableState.value = mutableState.value.copy(
                        isLoading = false,
                        error = error.message ?: "Unknown error",
                    )
                }
            )
        }
    }

    override fun clickOnGenre(genreName: String) {
        viewModelScope.launch {
            mutableEvents.emit(GenreListScreenEvents.NavigateToGenre(genreName))
        }
    }

    override fun updateSearchQuery(query: String) {
        val currentState = mutableState.value
        val filteredGenres = if (query.isBlank()) {
            currentState.genres
        } else {
            currentState.genres.filter { genre ->
                genre.name.contains(query, ignoreCase = true)
            }
        }
        mutableState.value = currentState.copy(
            searchQuery = query,
            filteredGenres = filteredGenres,
        )
    }

    override fun goBack() {
        viewModelScope.launch {
            mutableEvents.emit(GenreListScreenEvents.NavigateBack)
        }
    }

    interface Interactor {
        suspend fun getGenres(limit: Int = 100): Result<List<GenreData>>
    }

    data class GenreData(
        val name: String,
        val trackCount: Int,
    )
}
