package com.lelloman.pezzottify.android.domain.statics

import kotlinx.coroutines.flow.Flow

typealias StaticsItemFlow<T> = Flow<StaticsItem<out T>>

sealed interface StaticsItem<T> {

    val id: String

    data class Loading<T>(override val id: String) : StaticsItem<T>

    data class Error<T>(override val id: String, val error: Throwable) : StaticsItem<T>

    data class Loaded<T>(override val id: String, val data: T) : StaticsItem<T>
}