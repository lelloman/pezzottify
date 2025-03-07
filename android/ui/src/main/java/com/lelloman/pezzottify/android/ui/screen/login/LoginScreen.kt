package com.lelloman.pezzottify.android.ui.screen.login

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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

@Composable
fun LoginScreen() {
    val viewModel = remember { LoginViewModel() }
    LoginScreenInternal(
        state = viewModel.state.collectAsState().value,
        actions = viewModel,
    )
}

@Composable
private fun LoginScreenInternal(
    state: LoginScreenState,
    actions: LoginScreenActions,
) {
    var host by remember { mutableStateOf("") }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        OutlinedTextField(
            enabled = !state.isLoading,
            value = host,
            onValueChange = { host = it },
            label = { Text("Host") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 8.dp)
        )

        OutlinedTextField(
            enabled = !state.isLoading,
            value = email,
            onValueChange = { email = it },
            label = { Text("Email") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 8.dp)
        )

        OutlinedTextField(
            enabled = !state.isLoading,
            value = password,
            onValueChange = { password = it },
            label = { Text("Password") },
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 16.dp)
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
                Text("Login")
            }
        }
    }
}

@Preview
@Composable
private fun LoginPreview() {
    val coroutineScope = rememberCoroutineScope()

    var mutableState by remember {
        mutableStateOf(
            LoginScreenState(
                host = "http://10.0.2.2:3001",
                email = "william.henry.harrison@example-pet-store.com",
                password = "password",
                isLoading = false,
                hostError = null,
                emailError = null,
                error = null,
            )
        )
    }
    LoginScreenInternal(
        state = mutableState,
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
        }
    )
}