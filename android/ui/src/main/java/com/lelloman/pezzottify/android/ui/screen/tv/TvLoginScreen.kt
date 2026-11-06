package com.lelloman.pezzottify.android.ui.screen.tv

import android.Manifest
import android.os.Build
import android.widget.Toast
import androidx.activity.compose.BackHandler
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
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.ui.draw.scale
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.OutlinedButton
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
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import kotlinx.coroutines.delay
import androidx.compose.ui.platform.LocalContext
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.text.input.ImeAction
import androidx.annotation.StringRes
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.res.stringResource
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.screen.login.DeviceFlowState
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

    // "Sign in from your phone" is the default; the password form is the fallback.
    var usePassword by rememberSaveable { mutableStateOf(false) }
    val deviceFlowState by viewModel.deviceFlowState.collectAsState()

    // Poll only while the device flow is shown and the screen is visible.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner, usePassword) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_START -> if (!usePassword) viewModel.startDeviceFlow()
                Lifecycle.Event.ON_STOP -> viewModel.stopDeviceFlow()
                else -> Unit
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            viewModel.stopDeviceFlow()
        }
    }

    BackHandler(enabled = usePassword) { usePassword = false }

    val textFieldColors = OutlinedTextFieldDefaults.colors(
        focusedTextColor = MaterialTheme.colorScheme.onSurface,
        unfocusedTextColor = MaterialTheme.colorScheme.onSurface,
        focusedLabelColor = MaterialTheme.colorScheme.primary,
        unfocusedLabelColor = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    if (!usePassword) {
        TvDeviceFlowContent(
            state = deviceFlowState,
            isHostConfigurable = state.isHostConfigurable,
            host = state.host,
            hostErrorRes = state.hostErrorRes,
            textFieldColors = textFieldColors,
            onUpdateHost = viewModel::updateHost,
            onApplyHost = viewModel::applyDeviceFlowHost,
            onRestart = viewModel::restartDeviceFlow,
            onUsePassword = { usePassword = true },
        )
        return
    }

    TvLoginContent(
        state = state,
        textFieldColors = textFieldColors,
        onUpdateHost = viewModel::updateHost,
        onUpdateEmail = viewModel::updateEmail,
        onUpdatePassword = viewModel::updatePassword,
        onClickLogin = viewModel::clockOnLoginButton,
        onUseDeviceFlow = { usePassword = false },
    )
}

@Composable
private fun TvDeviceFlowContent(
    state: DeviceFlowState,
    isHostConfigurable: Boolean,
    host: String,
    @StringRes hostErrorRes: Int?,
    textFieldColors: androidx.compose.material3.TextFieldColors,
    onUpdateHost: (String) -> Unit,
    onApplyHost: () -> Unit,
    onRestart: () -> Unit,
    onUsePassword: () -> Unit,
) {
    val primaryActionFocus = remember { FocusRequester() }
    val usePasswordFocus = remember { FocusRequester() }
    val hasPrimaryAction = state is DeviceFlowState.Expired ||
        state is DeviceFlowState.Denied ||
        state is DeviceFlowState.Error

    // Give D-pad users an obvious starting point: the recovery action when there is one,
    // otherwise "Use password instead". The server field sits above and is one press up.
    LaunchedEffect(hasPrimaryAction) {
        runCatching {
            if (hasPrimaryAction) primaryActionFocus.requestFocus() else usePasswordFocus.requestFocus()
        }
    }

    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 64.dp, vertical = 24.dp),
        ) {
            Card(
                modifier = Modifier
                    .align(Alignment.Center)
                    .widthIn(max = 880.dp)
                    .fillMaxWidth(0.9f),
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer,
                ),
            ) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 32.dp, vertical = 24.dp),
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(bottom = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        TvLoginLogo()
                        Spacer(modifier = Modifier.weight(1f))
                        Text(
                            text = stringResource(R.string.device_flow_title),
                            style = MaterialTheme.typography.headlineSmall,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    }

                    // The Pezzottify server to sign in to; the OIDC issuer is fixed at build time.
                    // Pinned builds (server.baseUrl) have no server field at all.
                    if (isHostConfigurable) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(bottom = 16.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            OutlinedTextField(
                                value = host,
                                onValueChange = onUpdateHost,
                                label = { Text(stringResource(R.string.server_url)) },
                                singleLine = true,
                                keyboardOptions = KeyboardOptions(
                                    keyboardType = KeyboardType.Uri,
                                    imeAction = ImeAction.Done,
                                ),
                                keyboardActions = KeyboardActions(onDone = { onApplyHost() }),
                                isError = hostErrorRes != null,
                                supportingText = hostErrorRes?.let { { Text(stringResource(R.string.invalid_url_message)) } },
                                colors = textFieldColors,
                                modifier = Modifier.weight(1f),
                            )
                            Spacer(modifier = Modifier.width(16.dp))
                            DeviceFlowButton(
                                text = stringResource(R.string.device_flow_apply_host),
                                onClick = onApplyHost,
                                primary = false,
                            )
                        }
                    }

                    val usePasswordButton: @Composable () -> Unit = {
                        DeviceFlowButton(
                            text = stringResource(R.string.device_flow_use_password),
                            onClick = onUsePassword,
                            primary = false,
                            modifier = Modifier.focusRequester(usePasswordFocus),
                        )
                    }

                    when (state) {
                        DeviceFlowState.Idle, DeviceFlowState.Loading -> {
                            DeviceFlowProgress(stringResource(R.string.device_flow_loading))
                            Spacer(modifier = Modifier.height(8.dp))
                            usePasswordButton()
                        }

                        is DeviceFlowState.Waiting -> DeviceFlowWaiting(state, usePasswordButton)

                        DeviceFlowState.Approved -> DeviceFlowProgress(stringResource(R.string.device_flow_signing_in))

                        DeviceFlowState.Expired, DeviceFlowState.Denied, is DeviceFlowState.Error -> {
                            val (title, message, action) = when (state) {
                                DeviceFlowState.Expired -> Triple(
                                    stringResource(R.string.device_flow_expired),
                                    stringResource(R.string.device_flow_expired_message),
                                    stringResource(R.string.device_flow_new_code),
                                )

                                DeviceFlowState.Denied -> Triple(
                                    stringResource(R.string.device_flow_denied),
                                    stringResource(R.string.device_flow_denied_message),
                                    stringResource(R.string.device_flow_new_code),
                                )

                                else -> {
                                    val error = state as DeviceFlowState.Error
                                    Triple(
                                        stringResource(R.string.device_flow_error),
                                        if (error.invalidHost) stringResource(R.string.invalid_url_message)
                                        else error.message ?: stringResource(R.string.unknown_error),
                                        stringResource(R.string.retry),
                                    )
                                }
                            }
                            Text(
                                text = title,
                                style = MaterialTheme.typography.titleLarge,
                                color = MaterialTheme.colorScheme.onSurface,
                            )
                            Text(
                                text = message,
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(top = 8.dp, bottom = 24.dp),
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                                DeviceFlowButton(
                                    text = action,
                                    onClick = onRestart,
                                    primary = true,
                                    modifier = Modifier.focusRequester(primaryActionFocus),
                                )
                                usePasswordButton()
                            }
                        }
                    }
                }
            }
        }
    }
}

/** QR code first, then the link and the code to type, then the waiting hint and the fallback. */
@Composable
private fun DeviceFlowWaiting(
    state: DeviceFlowState.Waiting,
    usePasswordButton: @Composable () -> Unit,
) {
    // Ticks the remaining-time hint; the controller uses the same wall clock for expiresAtMs.
    var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(state.expiresAtMs) {
        while (true) {
            now = System.currentTimeMillis()
            delay(1_000)
        }
    }
    val remainingS = ((state.expiresAtMs - now).coerceAtLeast(0) + 999) / 1_000

    Row(verticalAlignment = Alignment.CenterVertically) {
        QrCodeImage(
            content = state.qrContent,
            contentDescription = stringResource(R.string.device_flow_qr_description),
            modifier = Modifier.size(216.dp),
        )
        Spacer(modifier = Modifier.width(32.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = stringResource(R.string.device_flow_instructions),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = state.verificationUri.removePrefix("https://").removePrefix("http://"),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.padding(vertical = 2.dp),
            )
            Text(
                text = stringResource(R.string.device_flow_enter_code),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = state.userCode,
                fontFamily = FontFamily.Monospace,
                fontWeight = FontWeight.Bold,
                fontSize = 48.sp,
                letterSpacing = 4.sp,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.padding(vertical = 4.dp),
            )
            Row(verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(
                    modifier = Modifier.size(18.dp),
                    strokeWidth = 2.dp,
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = stringResource(R.string.device_flow_waiting),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(modifier = Modifier.width(16.dp))
                Text(
                    text = stringResource(
                        R.string.device_flow_expires_in,
                        "%d:%02d".format(remainingS / 60, remainingS % 60),
                    ),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(modifier = Modifier.height(16.dp))
            usePasswordButton()
        }
    }
}

/** Button with an unmistakable focus ring and slight zoom for D-pad navigation. */
@Composable
private fun DeviceFlowButton(
    text: String,
    onClick: () -> Unit,
    primary: Boolean,
    modifier: Modifier = Modifier,
) {
    var focused by remember { mutableStateOf(false) }
    val scale by animateFloatAsState(
        targetValue = if (focused) 1.06f else 1f,
        animationSpec = tween(durationMillis = 120),
        label = "device-flow-button-scale",
    )
    val focusBorder = if (focused) BorderStroke(3.dp, MaterialTheme.colorScheme.onSurface) else null
    val buttonModifier = modifier
        .height(52.dp)
        .onFocusChanged { focused = it.isFocused }
        .scale(scale)
    if (primary) {
        Button(onClick = onClick, border = focusBorder, modifier = buttonModifier) {
            Text(text, style = MaterialTheme.typography.titleMedium)
        }
    } else {
        OutlinedButton(
            onClick = onClick,
            border = focusBorder ?: ButtonDefaults.outlinedButtonBorder(enabled = true),
            modifier = buttonModifier,
        ) {
            Text(text, style = MaterialTheme.typography.titleMedium)
        }
    }
}

@Composable
private fun DeviceFlowProgress(text: String) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.padding(vertical = 24.dp),
    ) {
        CircularProgressIndicator(modifier = Modifier.size(28.dp))
        Spacer(modifier = Modifier.width(16.dp))
        Text(
            text = text,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun TvLoginLogo(modifier: Modifier = Modifier) {
    Row(
        modifier = modifier,
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
}

@Composable
private fun TvLoginContent(
    state: LoginScreenState,
    textFieldColors: androidx.compose.material3.TextFieldColors,
    onUpdateHost: (String) -> Unit,
    onUpdateEmail: (String) -> Unit,
    onUpdatePassword: (String) -> Unit,
    onClickLogin: () -> Unit,
    onUseDeviceFlow: () -> Unit,
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
                    TvLoginLogo(modifier = Modifier.padding(bottom = 24.dp))

                    if (state.isHostConfigurable) {
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
                    }

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

                    OutlinedButton(
                        onClick = onUseDeviceFlow,
                        enabled = !state.isLoading,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 12.dp)
                            .height(48.dp),
                    ) {
                        Text(stringResource(R.string.device_flow_title))
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
                    host = "https://music.example.com",
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
                onUseDeviceFlow = {},
            )
        }
    }
}

@Preview(widthDp = 960, heightDp = 540, showBackground = true)
@Composable
private fun TvDeviceFlowPreview() {
    PezzottifyTheme(darkTheme = true, themeMode = ThemeMode.Dark) {
        TvDeviceFlowContent(
            isHostConfigurable = true,
            host = "https://music.example.com",
            hostErrorRes = null,
            textFieldColors = OutlinedTextFieldDefaults.colors(),
            onUpdateHost = {},
            onApplyHost = {},
            state = DeviceFlowState.Waiting(
                deviceCode = "dc_preview",
                userCode = "WDJB-MJHT",
                verificationUri = "https://auth.example.com/device",
                qrContent = "https://auth.example.com/device?user_code=WDJB-MJHT",
                expiresAtMs = System.currentTimeMillis() + 540_000,
                intervalMs = 5_000,
            ),
            onRestart = {},
            onUsePassword = {},
        )
    }
}
