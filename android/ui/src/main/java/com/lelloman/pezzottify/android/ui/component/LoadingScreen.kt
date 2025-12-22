package com.lelloman.pezzottify.android.ui.component

import androidx.compose.runtime.Composable
import androidx.compose.ui.tooling.preview.Preview
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme

@Composable
fun LoadingScreen() {
    PezzottifyLoader(size = LoaderSize.FullScreen)
}

@Composable
@Preview
private fun LoadingScreenPreview() {
    PezzottifyTheme {
        LoadingScreen()
    }
}
