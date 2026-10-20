package com.lelloman.pezzottify.android.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.Flow
import javax.inject.Inject

/** Replaces the normal shell as soon as any API call reports maintenance mode. */
@Composable
fun MaintenanceEffect(
    isOnMaintenanceScreen: () -> Boolean,
    onMaintenance: () -> Unit,
) {
    val viewModel = hiltViewModel<MaintenanceEventViewModel>()

    LaunchedEffect(Unit) {
        viewModel.events.collect {
            if (!isOnMaintenanceScreen()) {
                onMaintenance()
            }
        }
    }
}

@HiltViewModel
class MaintenanceEventViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel() {

    val events: Flow<Unit> = interactor.events()

    interface Interactor {
        fun events(): Flow<Unit>
    }
}
