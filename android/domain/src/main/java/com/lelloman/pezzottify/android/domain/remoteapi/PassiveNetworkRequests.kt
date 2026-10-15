package com.lelloman.pezzottify.android.domain.remoteapi

import kotlin.coroutines.AbstractCoroutineContextElement
import kotlin.coroutines.CoroutineContext

/** Background work may use an awake server but must never wake a sleeping one. */
object PassiveNetworkRequests : AbstractCoroutineContextElement(Key) {
    object Key : CoroutineContext.Key<PassiveNetworkRequests>
}
