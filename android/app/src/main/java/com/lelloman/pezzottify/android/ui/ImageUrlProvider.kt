package com.lelloman.pezzottify.android.ui

import com.lelloman.pezzottify.android.domain.statics.Image
import com.lelloman.pezzottify.android.domain.statics.ImageSize

/**
 * Utility object for selecting and prioritizing image URLs from Album and Artist image lists.
 *
 * Both Albums and Artists have two lists of images:
 * - Albums: covers (primary) and coverGroup (secondary)
 * - Artists: portraits (primary) and portraitGroup (secondary)
 *
 * This provider:
 * 1. Prioritizes the primary list over the secondary list
 * 2. Sorts images within each list by size preference
 * 3. Limits the number of URLs returned to avoid resource waste
 *
 * Matches the web implementation's image selection logic.
 */
object ImageUrlProvider {

    /**
     * Size preference for large images (detail screens, hero sections).
     * Prefers larger images for better quality.
     */
    private val BIG_IMAGE_SIZE_PREFS = mapOf(
        ImageSize.XLARGE to 5,
        ImageSize.LARGE to 4,
        ImageSize.DEFAULT to 2,
        ImageSize.SMALL to 1,
        ImageSize.MEDIUM to 0
    )

    /**
     * Size preference for small images (list items, thumbnails).
     * Prefers smaller images to save bandwidth and improve performance.
     */
    private val SMALL_IMAGE_SIZE_PREFS = mapOf(
        ImageSize.SMALL to 4,
        ImageSize.DEFAULT to 3,
        ImageSize.LARGE to 2,
        ImageSize.XLARGE to 1,
        ImageSize.MEDIUM to 0
    )

    /**
     * Selects and prioritizes image URLs from primary and secondary image lists.
     * Uses big image preferences (prefers larger sizes).
     *
     * @param baseUrl The base URL of the server (e.g., "http://10.0.2.2:3001")
     * @param primaryImages The primary list of images (covers or portraits)
     * @param secondaryImages The secondary list of images (coverGroup or portraitGroup)
     * @return List of image URLs prioritized by: primary list first, then by size preference
     */
    fun selectImageUrls(
        baseUrl: String,
        primaryImages: List<Image>,
        secondaryImages: List<Image>,
    ): List<String> {
        return selectImageUrlsInternal(
            baseUrl,
            primaryImages,
            secondaryImages,
            BIG_IMAGE_SIZE_PREFS
        )
    }

    /**
     * Selects and prioritizes image URLs optimized for small display contexts (lists, thumbnails).
     * Uses small image preferences (prefers smaller sizes for better performance).
     *
     * @param baseUrl The base URL of the server (e.g., "http://10.0.2.2:3001")
     * @param primaryImages The primary list of images (covers or portraits)
     * @param secondaryImages The secondary list of images (coverGroup or portraitGroup)
     * @return List of image URLs prioritized by: primary list first, then by size preference
     */
    fun selectSmallImageUrls(
        baseUrl: String,
        primaryImages: List<Image>,
        secondaryImages: List<Image>,
    ): List<String> {
        return selectImageUrlsInternal(
            baseUrl,
            primaryImages,
            secondaryImages,
            SMALL_IMAGE_SIZE_PREFS
        )
    }

    private fun selectImageUrlsInternal(
        baseUrl: String,
        primaryImages: List<Image>,
        secondaryImages: List<Image>,
        sizePrefs: Map<ImageSize, Int>
    ): List<String> {
        // Tag images as primary or secondary
        data class TaggedImage(val image: Image, val isPrimary: Boolean)

        val allImages = primaryImages.map { TaggedImage(it, true) } +
                secondaryImages.map { TaggedImage(it, false) }

        // Sort by: primary first, then by size preference
        val sortedImages = allImages.sortedWith(compareByDescending<TaggedImage> { it.isPrimary }
            .thenByDescending { sizePrefs[it.image.size] ?: 0 })

        return sortedImages
            .map { formatImageUrl(baseUrl, it.image.id) }
    }

    /**
     * Formats an image ID into a complete image URL.
     *
     * @param baseUrl The base URL of the server
     * @param imageId The image ID from the catalog
     * @return The complete URL for fetching the image
     */
    private fun formatImageUrl(baseUrl: String, imageId: String): String {
        // Remove trailing slash from baseUrl if present
        val cleanBaseUrl = baseUrl.trimEnd('/')
        return "$cleanBaseUrl/v1/content/image/$imageId"
    }
}
