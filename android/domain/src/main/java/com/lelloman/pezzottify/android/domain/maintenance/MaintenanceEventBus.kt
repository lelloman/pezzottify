package com.lelloman.pezzottify.android.domain.maintenance

import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Emitted when the server answers a player request with a maintenance 503.
 * The UI observes it and replaces the normal shell with the maintenance screen.
 */
@Singleton
class MaintenanceEventBus @Inject constructor() {

    private val _events = MutableSharedFlow<Unit>(extraBufferCapacity = 1)
    val events: SharedFlow<Unit> = _events.asSharedFlow()

    fun emit() {
        _events.tryEmit(Unit)
    }
}
