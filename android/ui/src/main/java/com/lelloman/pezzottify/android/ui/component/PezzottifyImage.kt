package com.lelloman.pezzottify.android.ui.component

import android.annotation.SuppressLint
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.dp
import coil3.compose.AsyncImage
import com.lelloman.pezzottify.android.ui.R

@Composable
fun PezzottifyImage(
    modifier: Modifier = Modifier,
    url: String,
    shape: PezzottifyImageShape = PezzottifyImageShape.SmallSquare,
    placeholder: PezzottifyImagePlaceholder = PezzottifyImagePlaceholder.GenericImage,
    contentDescription: String? = null
) {
    AsyncImage(
        model = url,
        contentDescription = contentDescription,
        modifier = shape.modifier(modifier),
        placeholder = rememberVectorPainter(placeholder.getIcon()),
        error = rememberVectorPainter(placeholder.getIcon()),
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