package com.lelloman.pezzottify.android.ui.content


sealed class SearchResultContent {
    abstract val id: String

    class Artist(override val id: String, val name: String, val imageUrl: String?) : SearchResultContent()

    class Album(
        override val id: String,
        val name: String,
        val artistNames: List<String>,
        val imageUrl: String?,
        val availability: AlbumAvailability = AlbumAvailability.Complete,
    ) : SearchResultContent()

    class Track(
        override val id: String,
        val name: String,
        val artistNames: List<String>,
        val durationSeconds: Int,
        val albumId: String,
        val albumImageUrl: String?,
        val availability: TrackAvailability = TrackAvailability.Available,
    ) : SearchResultContent() {
        val isPlayable: Boolean
            get() = availability.isPlayable

        val isFetching: Boolean
            get() = availability == TrackAvailability.Fetching

        val isFetchError: Boolean
            get() = availability == TrackAvailability.FetchError

        val isUnavailable: Boolean
            get() = !isPlayable
    }
}