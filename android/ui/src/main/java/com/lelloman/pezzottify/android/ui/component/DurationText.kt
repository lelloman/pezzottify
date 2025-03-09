package com.lelloman.pezzottify.android.ui.component

import android.annotation.SuppressLint
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

@Composable
fun DurationText(durationSeconds: Int, modifier: Modifier = Modifier) {
    val hours = durationSeconds / 3600
    val minutes = (durationSeconds % 3600) / 60
    val seconds = durationSeconds % 60

    @SuppressLint("DefaultLocale")
    val formattedDuration = String.format("%02d:%02d:%02d", hours, minutes, seconds)
    Text(formattedDuration, modifier = modifier)
}