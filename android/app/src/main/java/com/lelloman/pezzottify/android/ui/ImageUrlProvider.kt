package com.lelloman.pezzottify.android.ui

/**
 * Utility object for building image URLs from display image IDs.
 */
object ImageUrlProvider {

    /**
     * Builds an image URL from a display image ID.
     *
     * @param baseUrl The base URL of the server (e.g., "http://10.0.2.2:3001")
     * @param displayImageId The display image ID from the catalog, or null if no image
     * @return The image URL, or null if no display image ID is provided
     */
    fun buildImageUrl(baseUrl: String, displayImageId: String?): String? {
        if (displayImageId == null) return null
        val cleanBaseUrl = baseUrl.trimEnd('/')
        return "$cleanBaseUrl/v1/content/image/$displayImageId"
    }
}
