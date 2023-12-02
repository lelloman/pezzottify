package com.lelloman.pezzottify.android.app.ui.login

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TextFieldWithError(
    field: LoginViewModel.TextField,
    label: String,
    onValueChanged: (String) -> Unit,
    enabled: Boolean,
    isPassword: Boolean = false,
) {
    TextField(
        label = { Text(text = label) },
        value = field.value,
        onValueChange = onValueChanged,
        enabled = enabled,
        isError = field.hasError,
        supportingText = {
            field.error?.let { errorMsg ->
                Text(
                    text = errorMsg,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        },
        visualTransformation = if (isPassword) {
            PasswordVisualTransformation()
        } else {
            VisualTransformation.None
        },
        keyboardOptions = if (isPassword) {
            KeyboardOptions(keyboardType = KeyboardType.Password)
        } else {
            KeyboardOptions.Default
        },
    )
}

@Composable
fun LoginScreen(viewModel: LoginViewModel = hiltViewModel()) {
    val state by viewModel.state.collectAsState()
    val context = LocalContext.current
    Column(
        modifier = Modifier.padding(20.dp),
        verticalArrangement = Arrangement.Center,
        horizontalAlignment = Alignment.CenterHorizontally
    ) {

        TextFieldWithError(
            label = "Remote url",
            field = state.remoteUrl,
            onValueChanged = viewModel::onRemoteUrlUpdate,
            enabled = !state.loading,
        )

        Spacer(modifier = Modifier.height(20.dp))
        TextFieldWithError(
            label = "Username",
            field = state.username,
            onValueChanged = viewModel::onUsernameUpdate,
            enabled = !state.loading,
        )

        Spacer(modifier = Modifier.height(20.dp))
        TextFieldWithError(
            label = "Password",
            field = state.password,
            onValueChanged = viewModel::onPasswordUpdate,
            enabled = !state.loading,
            isPassword = true,
        )

        Spacer(modifier = Modifier.height(20.dp))
        Box(modifier = Modifier.padding(40.dp, 0.dp, 40.dp, 0.dp)) {
            Button(
                onClick = { viewModel.onLoginClicked() },
                shape = RoundedCornerShape(50.dp),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(50.dp),
                enabled = !state.loading,
            ) {
                Text(text = "Login")
            }
        }
    }
}
