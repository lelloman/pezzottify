package com.lelloman.pezzottify.android.ui.component

import android.annotation.SuppressLint
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.lelloman.pezzottify.android.ui.R

/**
 * Image component with multiple URL fallback support.
 *
 * Tries each URL in the list sequentially until one loads successfully.
 * If all URLs fail or the list is empty, displays the placeholder.
 *
 * @param urls List of image URLs to try in order
 * @param modifier Modifier for the image
 * @param shape Shape preset for the image (defines size/dimensions)
 * @param placeholder Placeholder icon to show while loading or on error
 * @param contentDescription Accessibility description
 */
@Composable
fun PezzottifyImage(
    urls: List<String>,
    modifier: Modifier = Modifier,
    shape: PezzottifyImageShape = PezzottifyImageShape.SmallSquare,
    placeholder: PezzottifyImagePlaceholder = PezzottifyImagePlaceholder.GenericImage,
    contentDescription: String? = null
) {
    var currentUrlIndex by remember(urls) { mutableIntStateOf(0) }

    if (urls.isEmpty() || currentUrlIndex >= urls.size) {
        // No URLs or all failed - show placeholder
        Image(
            painter = rememberVectorPainter(placeholder.getIcon()),
            contentDescription = contentDescription,
            modifier = shape.modifier(modifier)
        )
    } else {
        AsyncImage(
            model = urls[currentUrlIndex],
            contentDescription = contentDescription,
            modifier = shape.modifier(modifier),
            placeholder = rememberVectorPainter(placeholder.getIcon()),
            error = rememberVectorPainter(placeholder.getIcon()),
            onError = {
                // On error, try next URL
                if (currentUrlIndex < urls.size - 1) {
                    currentUrlIndex++
                }
            }
        )
    }
}

/**
 * Image component for a single URL.
 *
 * Convenience overload for backward compatibility when only one URL is available.
 *
 * @param url Single image URL
 * @param modifier Modifier for the image
 * @param shape Shape preset for the image (defines size/dimensions)
 * @param placeholder Placeholder icon to show while loading or on error
 * @param contentDescription Accessibility description
 */
@Composable
fun PezzottifyImage(
    url: String,
    modifier: Modifier = Modifier,
    shape: PezzottifyImageShape = PezzottifyImageShape.SmallSquare,
    placeholder: PezzottifyImagePlaceholder = PezzottifyImagePlaceholder.GenericImage,
    contentDescription: String? = null
) {
    PezzottifyImage(
        urls = listOf(url),
        modifier = modifier,
        shape = shape,
        placeholder = placeholder,
        contentDescription = contentDescription
    )
}

@SuppressLint("ModifierFactoryExtensionFunction")
sealed interface PezzottifyImageShape {

    fun modifier(modifier: Modifier): Modifier

    data object SmallSquare : PezzottifyImageShape {
        val size = 96.dp
        override fun modifier(modifier: Modifier) = modifier.size(size)
    }

    data object FullWidthPoster : PezzottifyImageShape {
        override fun modifier(modifier: Modifier) = modifier.fillMaxWidth()
    }
}

enum class PezzottifyImagePlaceholder {
    Head,
    GenericImage;

    @Composable
    fun getIcon () = when (this) {
        Head -> ImageVector.vectorResource(R.drawable.baseline_face_24)
        GenericImage -> ImageVector.vectorResource(R.drawable.baseline_image_24)
    }
}