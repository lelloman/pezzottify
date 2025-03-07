package com.lelloman.pezzottify.android.ui.screen.main.home

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.fromMainBackToLogin

@Composable
fun HomeScreen(modifier: Modifier = Modifier, parentNavController: NavController) {
    Box(modifier = modifier.fillMaxSize()) {
        Text(
            "HOME",
            modifier = Modifier.align(Alignment.Center),
            style = MaterialTheme.typography.headlineLarge
        )
        Button(
            modifier = Modifier.align(Alignment.BottomCenter),
            onClick = { parentNavController.fromMainBackToLogin() }) {
            Text("LOGOUT")
        }
    }
}

@Composable
@Preview
private fun HomeScreenPreview() {
    val navController = rememberNavController()
    HomeScreen(parentNavController = navController)
}