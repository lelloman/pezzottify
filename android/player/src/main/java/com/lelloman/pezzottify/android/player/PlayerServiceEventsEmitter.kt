package com.lelloman.pezzottify.android.player

import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch

internal class PlayerServiceEventsEmitter {

    private val mutableEvents = MutableSharedFlow<Event>()
    val events = mutableEvents.asSharedFlow()

    fun shutdown() {
        GlobalScope.launch {
            mutableEvents.emit(Event.Shutdown)
        }
    }

    enum class Event {
        Shutdown
    }
}