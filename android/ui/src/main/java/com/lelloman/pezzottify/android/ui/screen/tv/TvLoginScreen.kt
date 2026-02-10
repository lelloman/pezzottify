package com.lelloman.pezzottify.android.ui.screen.tv

import android.Manifest
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreenEvents
import com.lelloman.pezzottify.android.ui.screen.login.LoginScreenState
import com.lelloman.pezzottify.android.ui.screen.login.LoginViewModel
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import com.lelloman.pezzottify.android.ui.theme.ThemeMode
import com.lelloman.pezzottify.android.ui.tv.fromTvLoginToNowPlaying

@Composable
fun TvLoginScreen(navController: NavController) {
    val viewModel = hiltViewModel<LoginViewModel>()
    val state by viewModel.state.collectAsState()
    val context = LocalContext.current

    val notificationPermissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { /* no-op */ }

    LaunchedEffect(Unit) {
        viewModel.events.collect { event ->
            when (event) {
                LoginScreenEvents.NavigateToMain -> navController.fromTvLoginToNowPlaying()
                LoginScreenEvents.RequestNotificationPermission -> {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                }

                is LoginScreenEvents.OidcError -> {
                    Toast.makeText(context, event.message, Toast.LENGTH_LONG).show()
                }

                is LoginScreenEvents.LaunchOidcIntent -> {
                    // OIDC disabled on TV
                }
            }
        }
    }

    val textFieldColors = OutlinedTextFieldDefaults.colors(
        focusedTextColor = MaterialTheme.colorScheme.onSurface,
        unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
        focusedLabelColor = MaterialTheme.colorScheme.primary,
        unfocusedLabelColor = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    TvLoginContent(
        state = state,
        textFieldColors = textFieldColors,
        onUpdateHost = viewModel::updateHost,
        onUpdateEmail = viewModel::updateEmail,
        onUpdatePassword = viewModel::updatePassword,
        onClickLogin = viewModel::clockOnLoginButton,
    )
}

@Composable
private fun TvLoginContent(
    state: LoginScreenState,
    textFieldColors: androidx.compose.material3.TextFieldColors,
    onUpdateHost: (String) -> Unit,
    onUpdateEmail: (String) -> Unit,
    onUpdatePassword: (String) -> Unit,
    onClickLogin: () -> Unit,
) {

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 96.dp, vertical = 64.dp),
        ) {
            Card(
                modifier = Modifier
                    .align(Alignment.Center)
                    .widthIn(max = 560.dp)
                    .heightIn(max = 680.dp)
                    .fillMaxWidth(0.7f),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                ),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(32.dp),
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(bottom = 24.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            painter = painterResource(R.drawable.ic_pezzottify_logo),
                            contentDescription = null,
                            tint = Color.Unspecified,
                            modifier = Modifier
                                .size(40.dp)
                                .offset(x = 5.dp),
                        )
                        Text(
                            text = "ezzottify",
                            style = MaterialTheme.typography.headlineMedium,
                            color = MaterialTheme.colorScheme.primary,
                        )
                    }

                    OutlinedTextField(
                        enabled = !state.isLoading,
                        value = state.host,
                        onValueChange = onUpdateHost,
                        label = { Text(stringResource(R.string.server_url)) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 72.dp)
                            .padding(bottom = 16.dp),
                        isError = state.hostErrorRes != null,
                        supportingText = state.hostErrorRes?.let { { Text(stringResource(R.string.invalid_url_message)) } },
                        colors = textFieldColors,
                    )

                    OutlinedTextField(
                        enabled = !state.isLoading,
                        value = state.email,
                        onValueChange = onUpdateEmail,
                        label = { Text(stringResource(R.string.email)) },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 72.dp)
                            .padding(bottom = 16.dp),
                        colors = textFieldColors,
                    )

                    OutlinedTextField(
                        enabled = !state.isLoading,
                        value = state.password,
                        onValueChange = onUpdatePassword,
                        label = { Text(stringResource(R.string.password)) },
                        visualTransformation = PasswordVisualTransformation(),
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 72.dp)
                            .padding(bottom = 24.dp),
                        colors = textFieldColors,
                    )

                    if (state.isLoading) {
                        LinearProgressIndicator(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(6.dp),
                        )
                        Spacer(modifier = Modifier.height(16.dp))
                    }

                    Button(
                        onClick = onClickLogin,
                        enabled = !state.isLoading,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(56.dp),
                    ) {
                        Text(
                            stringResource(R.string.sign_in),
                            style = MaterialTheme.typography.titleLarge
                        )
                    }
                }
            }
        }
    }
}

@Preview(widthDp = 960, heightDp = 540, showBackground = true)
@Composable
private fun TvLoginScreenPreview() {
    PezzottifyTheme(darkTheme = true, themeMode = ThemeMode.Dark) {
        Surface(color = Color(0xFF101318)) {
            TvLoginContent(
                state = LoginScreenState(
                    host = "https://pezzottify.lelloman.com",
                    email = "demo-user",
                    password = "",
                    isLoading = false,
                ),
                textFieldColors = OutlinedTextFieldDefaults.colors(
                    focusedTextColor = MaterialTheme.colorScheme.onSurface,
                    unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
                    focusedLabelColor = MaterialTheme.colorScheme.primary,
                    unfocusedLabelColor = MaterialTheme.colorScheme.onSurfaceVariant,
                ),
                onUpdateHost = {},
                onUpdateEmail = {},
                onUpdatePassword = {},
                onClickLogin = {},
            )
        }
    }
}
