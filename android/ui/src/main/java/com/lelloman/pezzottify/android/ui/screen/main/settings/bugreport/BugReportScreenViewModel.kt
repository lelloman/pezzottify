package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class BugReportScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), BugReportScreenActions {

    private val mutableState = MutableStateFlow(BugReportScreenState())
    val state: StateFlow<BugReportScreenState> = mutableState.asStateFlow()

    override fun onTitleChanged(title: String) {
        mutableState.update { it.copy(title = title, errorRes = null, submitResult = null) }
    }

    override fun onDescriptionChanged(description: String) {
        mutableState.update { it.copy(description = description, errorRes = null, submitResult = null) }
    }

    override fun onIncludeLogsChanged(includeLogs: Boolean) {
        mutableState.update { it.copy(includeLogs = includeLogs) }
    }

    override fun submit() {
        val currentState = mutableState.value

        // Validate description is not empty
        if (currentState.description.isBlank()) {
            mutableState.update { it.copy(errorRes = R.string.bug_report_description_required) }
            return
        }

        if (currentState.isSubmitting) {
            return
        }

        mutableState.update { it.copy(isSubmitting = true, errorRes = null, submitResult = null) }

        viewModelScope.launch {
            val logs = if (currentState.includeLogs) {
                interactor.getLogs()
            } else {
                null
            }

            val result = interactor.submitBugReport(
                title = currentState.title.takeIf { it.isNotBlank() },
                description = currentState.description,
                logs = logs,
            )

            mutableState.update {
                it.copy(
                    isSubmitting = false,
                    submitResult = result,
                )
            }
        }
    }

    interface Interactor {
        fun getLogs(): String?
        suspend fun submitBugReport(
            title: String?,
            description: String,
            logs: String?,
        ): SubmitResult
    }
}
