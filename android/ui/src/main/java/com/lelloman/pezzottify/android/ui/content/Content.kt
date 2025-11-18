package com.lelloman.pezzottify.android.ui.content


sealed class Content<out T>() {

    abstract val itemId: String

    data class Loading(override val itemId: String) : Content<Nothing>()

    data class Error(override val itemId: String) : Content<Nothing>()

    data class Resolved<T>(override val itemId: String, val data: T) : Content<T>()
}