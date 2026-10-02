package com.lelloman.pezzottify.android.ui.component.destination

import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.dialog.DEFAULT_DESTINATION_STEPS
import com.lelloman.pezzottify.android.ui.component.dialog.SetDestinationDialog
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

/** An artist, album or track the user wants to steer the queue toward. */
data class DestinationRequest(
    val entityType: String,
    val entityId: String,
    val label: String,
)

@HiltViewModel
class PlaybackDestinationViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    fun isSteerable(): Boolean = interactor.isSteerable()

    fun currentStepsTotal(): Int = interactor.currentStepsTotal() ?: DEFAULT_DESTINATION_STEPS

    fun setDestination(request: DestinationRequest, steps: Int, onResult: (Boolean) -> Unit) {
        viewModelScope.launch {
            onResult(interactor.setDestination(request.entityType, request.entityId, request.label, steps))
        }
    }

    interface Interactor {
        fun isSteerable(): Boolean
        fun currentStepsTotal(): Int?
        suspend fun setDestination(entityType: String, entityId: String, label: String, steps: Int): Boolean
    }
}

/**
 * Shows the steps prompt for [request] and applies it to the current queue. Shows a hint
 * instead when nothing steerable is playing (nothing, or a radio).
 */
@Composable
fun PlaybackDestinationDialog(request: DestinationRequest?, onDismiss: () -> Unit) {
    if (request == null) return
    val viewModel = hiltViewModel<PlaybackDestinationViewModel>()
    val context = LocalContext.current
    if (!viewModel.isSteerable()) {
        LaunchedEffect(request) {
            Toast.makeText(context, R.string.steering_not_steerable, Toast.LENGTH_SHORT).show()
            onDismiss()
        }
        return
    }
    SetDestinationDialog(
        label = request.label,
        initialSteps = viewModel.currentStepsTotal(),
        onDismiss = onDismiss,
        onConfirm = { steps ->
            viewModel.setDestination(request, steps) { applied ->
                val message = if (applied) {
                    context.getString(R.string.steering_destination_set, request.label)
                } else {
                    context.getString(R.string.steering_not_steerable)
                }
                Toast.makeText(context, message, Toast.LENGTH_SHORT).show()
            }
        },
    )
}
