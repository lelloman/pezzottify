package com.lelloman.pezzottify.android.ui.screen.main.genre

interface GenreListScreenActions {
    fun clickOnGenre(genreName: String)
    fun updateSearchQuery(query: String)
    fun goBack()
}
