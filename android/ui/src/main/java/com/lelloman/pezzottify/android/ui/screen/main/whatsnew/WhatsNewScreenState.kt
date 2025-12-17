package com.lelloman.pezzottify.android.ui.screen.main.whatsnew

import com.lelloman.pezzottify.android.ui.content.Content
import kotlinx.coroutines.flow.Flow

data class WhatsNewScreenState(
    val batches: List<UiBatch> = emptyList(),
    val isLoading: Boolean = true,
    val error: String? = null,
)

data class UiBatch(
    val id: String,
    val name: String,
    val description: String?,
    val closedAt: Long,
    val summary: UiBatchSummary,
    val isExpanded: Boolean = false,
    val albums: List<Flow<Content<UiWhatsNewAlbum>>>? = null,
)

data class UiBatchSummary(
    val artistsAdded: Int,
    val albumsAdded: Int,
    val tracksAdded: Int,
    val artistsUpdated: Int,
    val albumsUpdated: Int,
    val tracksUpdated: Int,
)

data class UiWhatsNewAlbum(
    val id: String,
    val name: String,
    val imageUrl: String?,
)
