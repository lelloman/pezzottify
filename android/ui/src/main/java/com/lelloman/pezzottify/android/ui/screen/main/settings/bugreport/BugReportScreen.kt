package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import androidx.compose.foundation.layout.Column
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenScaffold
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.LoaderSize
import com.lelloman.pezzottify.android.ui.component.PezzottifyLoader
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

@Composable
fun BugReportScreen(navController: NavController) {
    val viewModel = hiltViewModel<BugReportScreenViewModel>()
    BugReportScreenInternal(
        state = viewModel.state,
        actions = viewModel,
        navController = navController,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun BugReportScreenInternal(
    state: StateFlow<BugReportScreenState>,
    actions: BugReportScreenActions,
    navController: NavController,
) {
    val currentState by state.collectAsState()

    MainScreenScaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.report_bug)) },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back)
                        )
                    }
                }
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp)
        ) {
            // Title field
            OutlinedTextField(
                value = currentState.title,
                onValueChange = actions::onTitleChanged,
                label = { Text(stringResource(R.string.bug_report_title)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                enabled = !currentState.isSubmitting,
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Description field
            OutlinedTextField(
                value = currentState.description,
                onValueChange = actions::onDescriptionChanged,
                label = { Text(stringResource(R.string.bug_report_description)) },
                placeholder = { Text(stringResource(R.string.bug_report_description_hint)) },
                modifier = Modifier
                    .fillMaxWidth()
                    .height(200.dp),
                enabled = !currentState.isSubmitting,
                isError = currentState.errorRes == R.string.bug_report_description_required,
            )

            // Show error if present
            currentState.errorRes?.let { errorRes ->
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    text = stringResource(errorRes),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error
                )
            }

            Spacer(modifier = Modifier.height(24.dp))

            // Include logs toggle
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = stringResource(R.string.bug_report_include_logs),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface
                    )
                    Text(
                        text = stringResource(R.string.bug_report_include_logs_description),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }
                Switch(
                    checked = currentState.includeLogs,
                    onCheckedChange = actions::onIncludeLogsChanged,
                    enabled = !currentState.isSubmitting,
                )
            }

            Spacer(modifier = Modifier.height(32.dp))

            // Submit button
            if (currentState.isSubmitting) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    PezzottifyLoader(size = LoaderSize.Small)
                    Spacer(modifier = Modifier.weight(1f))
                }
            } else {
                Button(
                    onClick = actions::submit,
                    modifier = Modifier.fillMaxWidth(),
                    enabled = currentState.submitResult !is SubmitResult.Success,
                ) {
                    Text(stringResource(R.string.bug_report_submit))
                }
            }

            // Show result
            currentState.submitResult?.let { result ->
                Spacer(modifier = Modifier.height(16.dp))
                val (text, color) = when (result) {
                    is SubmitResult.Success ->
                        stringResource(R.string.bug_report_success) to MaterialTheme.colorScheme.primary
                    is SubmitResult.Error ->
                        result.message to MaterialTheme.colorScheme.error
                }
                Text(
                    text = text,
                    style = MaterialTheme.typography.bodyMedium,
                    color = color
                )
            }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun BugReportScreenPreview() {
    PezzottifyTheme {
        BugReportScreenInternal(
            state = MutableStateFlow(BugReportScreenState()),
            actions = object : BugReportScreenActions {
                override fun onTitleChanged(title: String) {}
                override fun onDescriptionChanged(description: String) {}
                override fun onIncludeLogsChanged(includeLogs: Boolean) {}
                override fun submit() {}
            },
            navController = rememberNavController(),
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun BugReportScreenPreviewSubmitting() {
    PezzottifyTheme {
        BugReportScreenInternal(
            state = MutableStateFlow(
                BugReportScreenState(
                    title = "App crashes",
                    description = "When I click on an album, the app crashes",
                    isSubmitting = true,
                )
            ),
            actions = object : BugReportScreenActions {
                override fun onTitleChanged(title: String) {}
                override fun onDescriptionChanged(description: String) {}
                override fun onIncludeLogsChanged(includeLogs: Boolean) {}
                override fun submit() {}
            },
            navController = rememberNavController(),
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun BugReportScreenPreviewSuccess() {
    PezzottifyTheme {
        BugReportScreenInternal(
            state = MutableStateFlow(
                BugReportScreenState(
                    title = "App crashes",
                    description = "When I click on an album, the app crashes",
                    submitResult = SubmitResult.Success,
                )
            ),
            actions = object : BugReportScreenActions {
                override fun onTitleChanged(title: String) {}
                override fun onDescriptionChanged(description: String) {}
                override fun onIncludeLogsChanged(includeLogs: Boolean) {}
                override fun submit() {}
            },
            navController = rememberNavController(),
        )
    }
}
