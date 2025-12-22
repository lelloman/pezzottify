package com.lelloman.pezzottify.android.ui.component

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.theme.Spacing

/**
 * Size variants for the PezzottifyLoader component.
 */
enum class LoaderSize {
    /**
     * Full screen loader, centered in available space.
     * Use for initial screen loading states.
     */
    FullScreen,

    /**
     * Section loader with vertical padding.
     * Use for loading content within a section/list.
     */
    Section,

    /**
     * Inline loader matching thumbnail height.
     * Use for loading individual items in lists.
     */
    Inline,

    /**
     * Small loader for compact spaces.
     * Use inside buttons or small containers.
     */
    Small,

    /**
     * Medium loader for prominent loading states.
     * Use for download progress or action feedback.
     */
    Medium,

    /**
     * Button loader matching button text height.
     * Use inside buttons to indicate loading state.
     */
    Button,
}

/**
 * A consistent loading indicator component used throughout the app.
 *
 * @param size The size variant to use
 * @param modifier Optional modifier for customization
 * @param color Optional color for the indicator (uses theme default if not specified)
 */
@Composable
fun PezzottifyLoader(
    size: LoaderSize = LoaderSize.Section,
    modifier: Modifier = Modifier,
    color: Color = Color.Unspecified,
) {
    when (size) {
        LoaderSize.FullScreen -> {
            Box(
                modifier = modifier.fillMaxSize(),
                contentAlignment = Alignment.Center
            ) {
                if (color != Color.Unspecified) {
                    CircularProgressIndicator(color = color)
                } else {
                    CircularProgressIndicator()
                }
            }
        }

        LoaderSize.Section -> {
            Box(
                modifier = modifier
                    .fillMaxWidth()
                    .padding(vertical = Spacing.ExtraLarge),
                contentAlignment = Alignment.Center
            ) {
                if (color != Color.Unspecified) {
                    CircularProgressIndicator(color = color)
                } else {
                    CircularProgressIndicator()
                }
            }
        }

        LoaderSize.Inline -> {
            Box(
                modifier = modifier
                    .fillMaxWidth()
                    .height(48.dp),
                contentAlignment = Alignment.Center
            ) {
                if (color != Color.Unspecified) {
                    CircularProgressIndicator(
                        modifier = Modifier.size(24.dp),
                        strokeWidth = 2.dp,
                        color = color
                    )
                } else {
                    CircularProgressIndicator(
                        modifier = Modifier.size(24.dp),
                        strokeWidth = 2.dp
                    )
                }
            }
        }

        LoaderSize.Small -> {
            if (color != Color.Unspecified) {
                CircularProgressIndicator(
                    modifier = modifier.size(20.dp),
                    strokeWidth = 2.dp,
                    color = color
                )
            } else {
                CircularProgressIndicator(
                    modifier = modifier.size(20.dp),
                    strokeWidth = 2.dp
                )
            }
        }

        LoaderSize.Medium -> {
            if (color != Color.Unspecified) {
                CircularProgressIndicator(
                    modifier = modifier.size(48.dp),
                    color = color
                )
            } else {
                CircularProgressIndicator(
                    modifier = modifier.size(48.dp),
                )
            }
        }

        LoaderSize.Button -> {
            if (color != Color.Unspecified) {
                CircularProgressIndicator(
                    modifier = modifier.size(16.dp),
                    strokeWidth = 2.dp,
                    color = color
                )
            } else {
                CircularProgressIndicator(
                    modifier = modifier.size(16.dp),
                    strokeWidth = 2.dp
                )
            }
        }
    }
}

/**
 * A consistent loading indicator with custom fixed height.
 * Use when you need a specific container height.
 *
 * @param height The height of the loader container
 * @param modifier Optional modifier for customization
 */
@Composable
fun PezzottifyLoader(
    height: Dp,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(height),
        contentAlignment = Alignment.Center
    ) {
        CircularProgressIndicator(
            modifier = Modifier.size(24.dp),
            strokeWidth = 2.dp
        )
    }
}
