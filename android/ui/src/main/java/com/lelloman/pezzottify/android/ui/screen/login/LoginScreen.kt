package com.lelloman.pezzottify.android.ui.screen.login

import android.Manifest
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Lock
import androidx.compose.ui.platform.LocalContext
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.fromLoginToMain
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.launch

@Composable
fun LoginScreen(navController: NavController) {
    val viewModel = hiltViewModel<LoginViewModel>()

    LoginScreenInternal(
        state = viewModel.state.collectAsState().value,
        actions = viewModel,
        events = viewModel.events,
        navController = navController,
    )
}

@Composable
internal fun LoginScreenInternal(
    state: LoginScreenState,
    events: Flow<LoginScreenEvents>,
    actions: LoginScreenActions,
    navController: NavController,
    enableOidc: Boolean = true,
    onNavigateToMain: (() -> Unit)? = null,
) {
    val context = LocalContext.current

    // Permission launcher for POST_NOTIFICATIONS (Android 13+)
    val notificationPermissionLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.RequestPermission()
    ) { /* We don't need to handle the result - user can deny and still use the app */ }

    // OIDC auth launcher
    val oidcLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.StartActivityForResult()
    ) { /* Callback is handled via deep link in MainActivity */ }

    LaunchedEffect(Unit) {
        events.collect {
            when (it) {
                LoginScreenEvents.RequestNotificationPermission -> {
                    // Only request on Android 13+ (API 33)
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                        notificationPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
                    }
                }
                LoginScreenEvents.NavigateToMain -> {
                    onNavigateToMain?.invoke() ?: navController.fromLoginToMain()
                }
                is LoginScreenEvents.LaunchOidcIntent -> {
                    if (enableOidc) {
                        oidcLauncher.launch(it.intent)
                    }
                }
                is LoginScreenEvents.OidcError -> {
                    if (enableOidc) {
                        Toast.makeText(context, it.message, Toast.LENGTH_LONG).show()
                    }
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

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(16.dp),
            verticalArrangement = Arrangement.Center,
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            OutlinedTextField(
                enabled = !state.isLoading,
                value = state.host,
                onValueChange = actions::updateHost,
                label = { Text(stringResource(R.string.server_url)) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 8.dp),
                isError = state.hostErrorRes != null,
                supportingText = state.hostErrorRes?.let { { Text(stringResource(it)) } },
                colors = textFieldColors,
            )

            OutlinedTextField(
                enabled = !state.isLoading,
                value = state.email,
                onValueChange = actions::updateEmail,
                label = { Text(stringResource(R.string.email)) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 8.dp),
                colors = textFieldColors,
            )

            OutlinedTextField(
                enabled = !state.isLoading,
                value = state.password,
                onValueChange = actions::updatePassword,
                label = { Text(stringResource(R.string.password)) },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(bottom = 16.dp),
                colors = textFieldColors,
            )

            Box(modifier = Modifier.fillMaxWidth()) {
                val loaderAlpha: Float by animateFloatAsState(
                    targetValue = if (state.isLoading) 1f else 0f,
                    animationSpec = tween(
                        durationMillis = 200,
                        easing = LinearEasing,
                    )
                )

                LinearProgressIndicator(
                    modifier = Modifier
                        .height(4.dp)
                        .fillMaxWidth()
                        .align(Alignment.Center)
                        .alpha(loaderAlpha),
                    color = MaterialTheme.colorScheme.secondary,
                    trackColor = MaterialTheme.colorScheme.surfaceVariant,
                )
                Button(
                    onClick = {
                        if (!state.isLoading) {
                            actions.clockOnLoginButton()
                        }
                    },
                    modifier = Modifier
                        .fillMaxWidth()
                        .alpha(1f - loaderAlpha),
                ) {
                    Text(stringResource(R.string.login))
                }
            }

            if (enableOidc) {
                HorizontalDivider(
                    modifier = Modifier.padding(vertical = 16.dp),
                    color = MaterialTheme.colorScheme.outlineVariant,
                )

                OutlinedButton(
                    onClick = {
                        if (!state.isLoading) {
                            actions.clickOnOidcLogin()
                        }
                    },
                    enabled = !state.isLoading,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Icon(
                        imageVector = Icons.Default.Lock,
                        contentDescription = null,
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(stringResource(R.string.login_with_sso))
                }
            }
        }
    }
}

@Preview
@Composable
private fun LoginPreview() {
    val coroutineScope = rememberCoroutineScope()
    val navController = rememberNavController()

    var mutableState by remember {
        mutableStateOf(
            LoginScreenState(
                host = "http://10.0.2.2:3001",
                email = "william.henry.harrison@example-pet-store.com",
                password = "password",
                isLoading = false,
                hostErrorRes = null,
                emailErrorRes = null,
                errorRes = null,
            )
        )
    }
    PezzottifyTheme {
        LoginScreenInternal(
            state = mutableState,
            events = flow {},
            navController = navController,
            actions = object : LoginScreenActions {
                override fun updateHost(host: String) {
                    mutableState = mutableState.copy(host = host)
                }

                override fun updateEmail(email: String) {
                    mutableState = mutableState.copy(email = email)
                }

                override fun updatePassword(password: String) {
                    mutableState = mutableState.copy(password = password)
                }

                override fun clockOnLoginButton() {
                    if (!mutableState.isLoading) {
                        mutableState = mutableState.copy(isLoading = true)
                        coroutineScope.launch(Dispatchers.IO) {
                            delay(2000)
                            mutableState = mutableState.copy(isLoading = false)
                        }
                    }
                }

                override fun clickOnOidcLogin() {
                    if (!mutableState.isLoading) {
                        mutableState = mutableState.copy(isLoading = true)
                        coroutineScope.launch(Dispatchers.IO) {
                            delay(2000)
                            mutableState = mutableState.copy(isLoading = false)
                        }
                    }
                }
            }
        )
    }
}
