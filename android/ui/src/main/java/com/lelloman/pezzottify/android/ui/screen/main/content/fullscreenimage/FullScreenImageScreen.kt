package com.lelloman.pezzottify.android.ui.screen.main.content.fullscreenimage

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculateCentroid
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.navigation.NavController
import coil3.compose.AsyncImage
import com.lelloman.pezzottify.android.ui.R
import kotlinx.coroutines.launch
import kotlin.math.abs

private const val MIN_SCALE = 1f
private const val MAX_SCALE = 5f
private const val DISMISS_THRESHOLD = 0.3f
private const val DOUBLE_TAP_TIMEOUT_MS = 300L

@Composable
fun FullScreenImageScreen(
    imageUrl: String,
    navController: NavController,
) {
    var scale by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    var dismissOffsetY by remember { mutableFloatStateOf(0f) }
    var isDraggingToDismiss by remember { mutableStateOf(false) }

    val scope = rememberCoroutineScope()
    val density = LocalDensity.current
    val configuration = LocalConfiguration.current
    val screenHeightPx = with(density) { configuration.screenHeightDp.dp.toPx() }

    val dismissOffsetAnimatable = remember { Animatable(0f) }

    val backgroundAlpha = if (isDraggingToDismiss) {
        (1f - abs(dismissOffsetY) / screenHeightPx).coerceIn(0f, 1f)
    } else {
        1f
    }

    fun dismiss() {
        navController.popBackStack()
    }

    fun animateSnapBack() {
        scope.launch {
            dismissOffsetAnimatable.snapTo(dismissOffsetY)
            dismissOffsetAnimatable.animateTo(
                0f,
                animationSpec = spring(stiffness = Spring.StiffnessMedium)
            ) {
                dismissOffsetY = value
            }
        }
    }

    BackHandler {
        dismiss()
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black.copy(alpha = backgroundAlpha))
            .pointerInput(Unit) {
                var lastTapTime = 0L
                var lastTapPosition = Offset.Zero

                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    val downTime = System.currentTimeMillis()
                    val downPosition = down.position

                    var isDismissDrag = false
                    var isZoomPan = false
                    var totalDragY = 0f
                    var pointerCount = 1

                    while (true) {
                        val event = awaitPointerEvent()

                        when (event.type) {
                            PointerEventType.Move -> {
                                val activePointers = event.changes.filter { it.pressed }
                                pointerCount = activePointers.size

                                if (pointerCount >= 2) {
                                    // Multi-touch: zoom and pan
                                    isZoomPan = true
                                    isDismissDrag = false

                                    val zoom = event.calculateZoom()
                                    val pan = event.calculatePan()

                                    val newScale = (scale * zoom).coerceIn(MIN_SCALE, MAX_SCALE)

                                    if (newScale > 1f) {
                                        val maxOffsetX = (size.width * (newScale - 1)) / 2
                                        val maxOffsetY = (size.height * (newScale - 1)) / 2
                                        offset = Offset(
                                            x = (offset.x + pan.x * newScale).coerceIn(-maxOffsetX, maxOffsetX),
                                            y = (offset.y + pan.y * newScale).coerceIn(-maxOffsetY, maxOffsetY)
                                        )
                                    } else {
                                        offset = Offset.Zero
                                    }

                                    scale = newScale

                                    event.changes.forEach { if (it.positionChanged()) it.consume() }
                                } else if (pointerCount == 1 && !isZoomPan) {
                                    val change = activePointers.first()
                                    val dragDelta = change.position - change.previousPosition

                                    if (scale > 1f) {
                                        // Zoomed in: pan the image
                                        val maxOffsetX = (size.width * (scale - 1)) / 2
                                        val maxOffsetY = (size.height * (scale - 1)) / 2
                                        offset = Offset(
                                            x = (offset.x + dragDelta.x).coerceIn(-maxOffsetX, maxOffsetX),
                                            y = (offset.y + dragDelta.y).coerceIn(-maxOffsetY, maxOffsetY)
                                        )
                                        change.consume()
                                    } else {
                                        // At 1x scale: vertical drag to dismiss
                                        totalDragY += dragDelta.y

                                        if (abs(totalDragY) > 10f || isDismissDrag) {
                                            isDismissDrag = true
                                            isDraggingToDismiss = true
                                            dismissOffsetY += dragDelta.y
                                            change.consume()
                                        }
                                    }
                                }
                            }

                            PointerEventType.Release -> {
                                val activePointers = event.changes.filter { it.pressed }
                                if (activePointers.isEmpty()) {
                                    // All pointers released
                                    if (isDismissDrag) {
                                        isDraggingToDismiss = false
                                        val dismissThresholdPx = screenHeightPx * DISMISS_THRESHOLD
                                        if (abs(dismissOffsetY) > dismissThresholdPx) {
                                            dismiss()
                                        } else {
                                            animateSnapBack()
                                        }
                                    } else if (!isZoomPan && pointerCount == 1) {
                                        // Check for tap or double-tap
                                        val timeSinceLastTap = downTime - lastTapTime
                                        val distanceFromLastTap = (downPosition - lastTapPosition).getDistance()

                                        if (timeSinceLastTap < DOUBLE_TAP_TIMEOUT_MS && distanceFromLastTap < 100f) {
                                            // Double tap
                                            if (scale > 1f) {
                                                scale = 1f
                                                offset = Offset.Zero
                                            } else {
                                                scale = 2.5f
                                                val centerX = size.width / 2f
                                                val centerY = size.height / 2f
                                                offset = Offset(
                                                    x = (centerX - downPosition.x) * (scale - 1),
                                                    y = (centerY - downPosition.y) * (scale - 1)
                                                )
                                            }
                                            lastTapTime = 0L
                                        } else {
                                            // Single tap - store for potential double tap
                                            lastTapTime = downTime
                                            lastTapPosition = downPosition

                                            // Delay dismiss to check for double tap
                                            if (scale == 1f && abs(totalDragY) < 10f) {
                                                scope.launch {
                                                    kotlinx.coroutines.delay(DOUBLE_TAP_TIMEOUT_MS)
                                                    if (lastTapTime == downTime) {
                                                        dismiss()
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    break
                                }
                            }

                            PointerEventType.Exit -> break
                        }
                    }
                }
            }
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
                translationX = offset.x
                translationY = offset.y + dismissOffsetY
            },
        contentAlignment = Alignment.Center
    ) {
        AsyncImage(
            model = imageUrl,
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = Modifier.fillMaxSize(),
            error = painterResource(R.drawable.baseline_image_24),
        )
    }
}
