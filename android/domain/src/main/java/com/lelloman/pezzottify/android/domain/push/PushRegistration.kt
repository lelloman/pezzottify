package com.lelloman.pezzottify.android.domain.push

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** A UnifiedPush distributor app installed on the device. */
data class PushDistributor(val packageName: String, val label: String)

/** Where the push wake-up registration stands. See docs/unifiedpush.md. */
sealed interface PushStatus {
    /** Not logged in: nothing to register. */
    data object LoggedOut : PushStatus

    /** The user turned push off; background sync still delivers notifications. */
    data object Disabled : PushStatus

    /** No UnifiedPush distributor is installed. */
    data object NoDistributor : PushStatus

    /** Several distributors are installed and none is chosen yet. */
    data object ChooseDistributor : PushStatus

    /** The server has push disabled (HTTP 503). */
    data object ServerUnsupported : PushStatus

    /** Waiting for the distributor's endpoint or the server's confirmation. */
    data class Registering(val distributor: PushDistributor) : PushStatus

    data class Registered(val distributor: PushDistributor) : PushStatus

    data class Failed(val distributor: PushDistributor?, val reason: String) : PushStatus
}

data class PushState(
    val enabled: Boolean,
    val status: PushStatus,
    val distributors: List<PushDistributor>,
)

/**
 * Registration of this device for push wake-ups. A push only triggers a sync catch-up;
 * notifications keep coming from synced events.
 */
interface PushRegistration {
    val state: StateFlow<PushState>

    fun setEnabled(enabled: Boolean)

    fun chooseDistributor(packageName: String)

    /** Re-evaluate distributors and registration (e.g. when the app comes to the foreground). */
    fun refresh()

    /** Remove this device's registration while the session credential is still valid. */
    suspend fun unregisterForLogout()
}

/** Used where push is not available (tests, builds without an implementation). */
object NoPushRegistration : PushRegistration {
    private val mutableState = MutableStateFlow(PushState(false, PushStatus.Disabled, emptyList()))
    override val state: StateFlow<PushState> = mutableState.asStateFlow()
    override fun setEnabled(enabled: Boolean) = Unit
    override fun chooseDistributor(packageName: String) = Unit
    override fun refresh() = Unit
    override suspend fun unregisterForLogout() = Unit
}
