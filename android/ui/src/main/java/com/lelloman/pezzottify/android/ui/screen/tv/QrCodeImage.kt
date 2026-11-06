package com.lelloman.pezzottify.android.ui.screen.tv

import android.graphics.Bitmap
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.common.BitMatrix
import com.google.zxing.qrcode.QRCodeWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel

/**
 * Dark-on-white QR code for [content], always on a white plate (with quiet zone) so phones can
 * scan it on a dark theme. The bitmap has one pixel per module and is scaled up without filtering.
 */
@Composable
fun QrCodeImage(
    content: String,
    contentDescription: String?,
    modifier: Modifier = Modifier,
) {
    val bitmap = remember(content) { runCatching { qrCodeBitmap(content) }.getOrNull() }
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .background(Color.White)
            .padding(12.dp),
    ) {
        if (bitmap != null) {
            Image(
                bitmap = bitmap,
                contentDescription = contentDescription,
                contentScale = ContentScale.Fit,
                filterQuality = FilterQuality.None,
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

/** Encodes [content] as a QR code matrix without its own margin (the white plate provides it). */
internal fun qrCodeMatrix(content: String): BitMatrix =
    QRCodeWriter().encode(
        content,
        BarcodeFormat.QR_CODE,
        0,
        0,
        mapOf(
            EncodeHintType.MARGIN to 0,
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.M,
            EncodeHintType.CHARACTER_SET to "UTF-8",
        ),
    )

private fun qrCodeBitmap(content: String): ImageBitmap {
    val matrix = qrCodeMatrix(content)
    val width = matrix.width
    val height = matrix.height
    val pixels = IntArray(width * height) { index ->
        if (matrix[index % width, index / width]) android.graphics.Color.BLACK else android.graphics.Color.WHITE
    }
    return Bitmap.createBitmap(pixels, width, height, Bitmap.Config.ARGB_8888).asImageBitmap()
}
