package com.lelloman.pezzottify.android.ui.screen.about

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

@Composable
fun AboutScreen(state: AboutScreenState) {

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp)
            .verticalScroll(rememberScrollState())
    ) {
        Text(state.appName, style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.padding(4.dp))
        Text(stringResource(R.string.version, state.version))
        Text(stringResource(R.string.commit, state.commit))
    }
}

@Composable
@Preview
fun AboutScreenPreview() {
    AboutScreen(
        AboutScreenState(
            appName = "Pezzottify",
            version = "1.0.0",
            commit = "abcdef"
        )
    )
}