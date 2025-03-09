package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.lelloman.pezzottify.android.ui.R

@Composable
fun SquarePezzottifyImage(
    modifier: Modifier = Modifier,
    url: String,
    size: SquarePezzottifyImageSize = SquarePezzottifyImageSize.Small,
    placeholder: PezzottifyImagePlaceholder = PezzottifyImagePlaceholder.GenericImage,
    contentDescription: String? = null
) {
    AsyncImage(
        model = url,
        contentDescription = contentDescription,
        modifier = modifier.size(size.value),
        placeholder = rememberVectorPainter(placeholder.getIcon()),
        error = rememberVectorPainter(placeholder.getIcon()),
    )
}

enum class SquarePezzottifyImageSize(val value: Dp) {
    Small(96.dp)
}

enum class PezzottifyImagePlaceholder {
    Head,
    GenericImage;

    @Composable
    fun getIcon() = when (this) {
        Head -> ImageVector.vectorResource(R.drawable.baseline_face_24)
        GenericImage -> ImageVector.vectorResource(R.drawable.baseline_image_24)
    }
}