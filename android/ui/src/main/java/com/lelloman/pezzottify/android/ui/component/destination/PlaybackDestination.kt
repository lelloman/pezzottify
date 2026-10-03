package com.lelloman.pezzottify.android.ui.component.destination

import android.widget.Toast
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.platform.LocalContext
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.dialog.DEFAULT_DESTINATION_STEPS
import com.lelloman.pezzottify.android.ui.component.dialog.SetDestinationDialog
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

/**
 * An artist, album or track the user wants to steer the queue toward. With [addToMix] it joins
 * the current destination mix instead of replacing it, and no steps prompt is shown.
 */
data class DestinationRequest(
    val entityType: String,
    val entityId: String,
    val label: String,
    val addToMix: Boolean = false,
)

@HiltViewModel
class PlaybackDestinationViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    /** Whether the controlled queue already has a destination mix to add to. */
    val hasDestination: StateFlow<Boolean> =
        interactor.observeHasDestination().stateIn(viewModelScope, SharingStarted.Eagerly, false)

    fun isSteerable(): Boolean = interactor.isSteerable()

    fun currentStepsTotal(): Int = interactor.currentStepsTotal() ?: DEFAULT_DESTINATION_STEPS

    fun setDestination(request: DestinationRequest, steps: Int, onResult: (Boolean) -> Unit) {
        viewModelScope.launch {
            onResult(interactor.setDestination(request.entityType, request.entityId, request.label, steps))
        }
    }

    fun addToDestination(request: DestinationRequest, onResult: (Boolean) -> Unit) {
        viewModelScope.launch {
            onResult(interactor.addToDestination(request.entityType, request.entityId, request.label))
        }
    }

    interface Interactor {
        fun isSteerable(): Boolean
        fun currentStepsTotal(): Int?
        fun observeHasDestination(): Flow<Boolean>
        suspend fun setDestination(entityType: String, entityId: String, label: String, steps: Int): Boolean
        suspend fun addToDestination(entityType: String, entityId: String, label: String): Boolean
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
    if (request.addToMix) {
        LaunchedEffect(request) {
            viewModel.addToDestination(request) { applied ->
                val message = if (applied) {
                    context.getString(R.string.steering_destination_added, request.label)
                } else {
                    context.getString(R.string.steering_not_steerable)
                }
                Toast.makeText(context, message, Toast.LENGTH_SHORT).show()
            }
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

/** True while the controlled queue is being steered, so "Add to playback destination" makes sense. */
@Composable
fun rememberHasPlaybackDestination(): Boolean {
    val viewModel = hiltViewModel<PlaybackDestinationViewModel>()
    val hasDestination by viewModel.hasDestination.collectAsState()
    return hasDestination
}
