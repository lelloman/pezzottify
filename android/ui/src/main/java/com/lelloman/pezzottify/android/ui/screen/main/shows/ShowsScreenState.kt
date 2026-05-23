package com.lelloman.pezzottify.android.ui.screen.main.shows

data class ShowsScreenState(
    val isLoading: Boolean = false,
    val shows: List<ShowSummaryItem> = emptyList(),
    val selectedShow: ShowDetailItem? = null,
    val error: String? = null,
)

data class ShowSummaryItem(
    val id: String,
    val title: String,
    val summary: String,
    val targetDurationMinutes: Int,
    val trackCount: Int,
)

data class ShowDetailItem(
    val id: String,
    val title: String,
    val summary: String,
    val segments: List<ShowSegmentItem>,
)

data class ShowSegmentItem(
    val id: String,
    val kind: String,
    val title: String,
    val trackId: String? = null,
    val text: String? = null,
)
