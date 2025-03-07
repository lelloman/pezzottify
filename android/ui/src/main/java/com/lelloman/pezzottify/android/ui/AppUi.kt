package com.lelloman.pezzottify.android.ui

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.ParagraphStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.Preview

@Composable
fun AppUi() {
    Text(
        "AppUI",
        modifier = Modifier
            .fillMaxSize()
            .wrapContentHeight(),
        style = MaterialTheme.typography.displayLarge.plus(
            ParagraphStyle(textAlign = TextAlign.Center)
        )
    )
}

@Preview
@Composable
fun AppUiPreview() {
    AppUi()
}