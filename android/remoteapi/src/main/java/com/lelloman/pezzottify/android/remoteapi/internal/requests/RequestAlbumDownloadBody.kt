package com.lelloman.pezzottify.android.remoteapi.internal.requests

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * Request body for POST /v1/download/request/album.
 */
@Serializable
data class RequestAlbumDownloadBody(
    /** External album ID from the music provider */
    @SerialName("album_id")
    val albumId: String,
    /** Album name for display */
    @SerialName("album_name")
    val albumName: String,
    /** Artist name for display */
    @SerialName("artist_name")
    val artistName: String,
)
