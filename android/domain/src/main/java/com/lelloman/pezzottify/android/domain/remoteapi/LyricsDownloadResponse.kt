package com.lelloman.pezzottify.android.domain.remoteapi

import kotlinx.serialization.Serializable

@Serializable
data class LyricsDownloadResponse(val status: String, val tracks: Int)
