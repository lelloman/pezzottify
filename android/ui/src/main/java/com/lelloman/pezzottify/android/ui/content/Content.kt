package com.lelloman.pezzottify.android.ui.content


sealed class Content<out T>(val itemId: String) {

    class Loading(itemId: String) : Content<Nothing>(itemId)

    class Error(itemId: String) : Content<Nothing>(itemId)

    class Resolved<T>(itemId: String, val data: T) : Content<T>(itemId)
}