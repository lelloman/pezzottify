package com.lelloman.pezzottify.android.push

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import com.lelloman.pezzottify.android.domain.push.PushDistributor
import com.lelloman.pezzottify.android.domain.push.PushRegistration
import com.lelloman.pezzottify.android.domain.push.PushState
import com.lelloman.pezzottify.android.domain.push.PushStatus
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import javax.inject.Inject
import javax.inject.Named
import javax.inject.Singleton

/** Outcome of a server-side registration step run by a worker. */
enum class ServerSyncResult { Done, Retry }

/**
 * UnifiedPush registration (docs/unifiedpush.md). A push only wakes the app to run its
 * normal sync catch-up; without a distributor the app keeps polling.
 */
@Singleton
class UnifiedPushRegistration @Inject constructor(
    private val client: UnifiedPushClient,
    private val prefs: PushPreferences,
    private val scheduler: PushWorkScheduler,
    private val authStore: AuthStore,
    private val remoteApiClient: RemoteApiClient,
    private val deviceInfoProvider: DeviceInfoProvider,
    private val notifications: SystemNotificationHelper,
    @Named(PUSH_SCOPE) private val scope: CoroutineScope,
    loggerFactory: LoggerFactory,
) : PushRegistration, AppInitializer {

    private val logger: Logger by loggerFactory
    private val mutex = Mutex()
    private val mutableState = MutableStateFlow(
        PushState(enabled = prefs.enabled, status = PushStatus.LoggedOut, distributors = emptyList())
    )
    override val state: StateFlow<PushState> = mutableState.asStateFlow()

    override fun initialize() {
        scope.launch {
            authStore.getAuthState()
                .map { it is AuthState.LoggedIn }
                .distinctUntilChanged()
                .collect { mutex.withLock { evaluate() } }
        }
    }

    override fun refresh() {
        scope.launch { mutex.withLock { evaluate() } }
    }

    override fun setEnabled(enabled: Boolean) {
        scope.launch {
            mutex.withLock {
                prefs.enabled = enabled
                if (!enabled) dropRegistration()
                evaluate()
            }
        }
    }

    override fun chooseDistributor(packageName: String) {
        scope.launch {
            mutex.withLock {
                if (prefs.registeredDistributor != packageName) dropRegistration()
                client.saveDistributor(packageName)
                evaluate()
            }
        }
    }

    override suspend fun unregisterForLogout() {
        mutex.withLock {
            (prefs.serverEndpoint ?: prefs.endpoint)?.let { endpoint ->
                val result = remoteApiClient.deletePushRegistration(endpoint)
                if (result !is RemoteApiResponse.Success) {
                    logger.warn("Push registration removal on logout failed: $result")
                }
            }
            if (prefs.endpoint != null || prefs.registeredDistributor != null) client.unregister()
            prefs.clearRegistration()
            publish(PushStatus.LoggedOut, distributors())
        }
    }

    // --- Distributor callbacks (PushServiceImpl) ---

    fun onNewEndpoint(url: String, p256dh: String?, auth: String?) {
        scope.launch {
            mutex.withLock {
                val distributor = currentDistributor()
                if (p256dh.isNullOrBlank() || auth.isNullOrBlank()) {
                    publish(PushStatus.Failed(distributor, "Distributor sent no encryption keys"))
                    return@withLock
                }
                val previous = prefs.serverEndpoint
                prefs.endpoint = url
                prefs.p256dh = p256dh
                prefs.auth = auth
                if (previous != null && previous != url) {
                    scheduler.scheduleServerDeletion(previous)
                    prefs.serverEndpoint = null
                }
                if (prefs.serverEndpoint == url) {
                    distributor?.let { publish(PushStatus.Registered(it)) }
                } else {
                    scheduler.scheduleServerRegistration()
                    distributor?.let { publish(PushStatus.Registering(it)) }
                }
            }
        }
    }

    fun onMessage(content: ByteArray) {
        when (val payload = PushPayload.parse(content)) {
            is PushPayload.Test -> notifications.showTestPushNotification(payload.title, payload.body)
            PushPayload.Wake -> scheduler.schedulePushSync()
        }
    }

    fun onUnregistered() {
        scope.launch {
            mutex.withLock {
                val distributor = currentDistributor()
                (prefs.serverEndpoint ?: prefs.endpoint)?.let(scheduler::scheduleServerDeletion)
                prefs.clearRegistration()
                // Not re-registering here: the distributor (or its user) dropped us. The next
                // refresh, e.g. at app start, registers again.
                publish(PushStatus.Failed(distributor, "Unregistered by the distributor"))
            }
        }
    }

    fun onRegistrationFailed(reason: String) {
        scope.launch {
            mutex.withLock {
                logger.warn("UnifiedPush registration failed: $reason")
                prefs.registeredDistributor = null
                publish(PushStatus.Failed(currentDistributor(), reason))
            }
        }
    }

    // --- Server side (workers) ---

    suspend fun registerOnServer(): ServerSyncResult = mutex.withLock {
        val endpoint = prefs.endpoint
        val p256dh = prefs.p256dh
        val auth = prefs.auth
        if (endpoint == null || p256dh == null || auth == null || !isLoggedIn() || !prefs.enabled) {
            return@withLock ServerSyncResult.Done
        }
        val deviceId = runCatching { deviceInfoProvider.getDeviceInfo().deviceUuid }.getOrNull()
        when (val result = remoteApiClient.putPushRegistration(endpoint, p256dh, auth, deviceId)) {
            is RemoteApiResponse.Success -> {
                prefs.serverEndpoint = endpoint
                currentDistributor()?.let { publish(PushStatus.Registered(it)) }
                ServerSyncResult.Done
            }
            is RemoteApiResponse.Error -> when {
                result.isServerDisabled() -> {
                    publish(PushStatus.ServerUnsupported)
                    ServerSyncResult.Done
                }
                result is RemoteApiResponse.Error.Unauthorized -> ServerSyncResult.Done
                else -> {
                    logger.warn("Push registration on server failed, will retry: $result")
                    ServerSyncResult.Retry
                }
            }
        }
    }

    suspend fun deleteOnServer(endpoint: String): ServerSyncResult {
        if (!isLoggedIn()) return ServerSyncResult.Done
        return when (val result = remoteApiClient.deletePushRegistration(endpoint)) {
            is RemoteApiResponse.Success -> ServerSyncResult.Done
            RemoteApiResponse.Error.Network -> ServerSyncResult.Retry
            is RemoteApiResponse.Error.Unknown ->
                if (result.isServerDisabled()) ServerSyncResult.Done else ServerSyncResult.Retry
            else -> ServerSyncResult.Done
        }
    }

    // --- Internals (call with the mutex held) ---

    private suspend fun evaluate() {
        val distributors = distributors()
        if (!isLoggedIn()) return publish(PushStatus.LoggedOut, distributors)
        if (!prefs.enabled) return publish(PushStatus.Disabled, distributors)
        if (distributors.isEmpty()) {
            dropRegistration()
            return publish(PushStatus.NoDistributor, distributors)
        }
        val packages = distributors.map { it.packageName }
        val chosen = client.savedDistributor()?.takeIf { it in packages }
            ?: packages.singleOrNull()
            ?: client.defaultDistributor()?.takeIf { it in packages }
            ?: return publish(PushStatus.ChooseDistributor, distributors)
        val distributor = distributors.first { it.packageName == chosen }

        val vapid = when (val result = remoteApiClient.getPushVapidKey()) {
            is RemoteApiResponse.Success -> result.data
            is RemoteApiResponse.Error -> {
                return if (result.isServerDisabled()) {
                    publish(PushStatus.ServerUnsupported, distributors)
                } else if (prefs.serverEndpoint != null && prefs.serverEndpoint == prefs.endpoint) {
                    // Offline: keep the working registration.
                    publish(PushStatus.Registered(distributor), distributors)
                } else {
                    publish(PushStatus.Failed(distributor, "Server unreachable"), distributors)
                }
            }
        }

        if (prefs.registeredDistributor != chosen || prefs.vapidKey != vapid || prefs.endpoint == null) {
            if (prefs.registeredDistributor != null && prefs.registeredDistributor != chosen) {
                client.unregister()
            }
            client.saveDistributor(chosen)
            client.register(vapid)
            prefs.registeredDistributor = chosen
            prefs.vapidKey = vapid
            return publish(PushStatus.Registering(distributor), distributors)
        }
        if (prefs.serverEndpoint == prefs.endpoint) {
            publish(PushStatus.Registered(distributor), distributors)
        } else {
            scheduler.scheduleServerRegistration()
            publish(PushStatus.Registering(distributor), distributors)
        }
    }

    /** Unregister from the distributor and the server, keeping the distributor choice. */
    private fun dropRegistration() {
        val endpoint = prefs.serverEndpoint ?: prefs.endpoint
        if (endpoint != null) scheduler.scheduleServerDeletion(endpoint)
        if (prefs.registeredDistributor != null || prefs.endpoint != null) client.unregister()
        prefs.clearRegistration()
    }

    private fun distributors(): List<PushDistributor> = client.distributors()
        .map { PushDistributor(it, client.label(it)) }

    private fun currentDistributor(): PushDistributor? =
        (prefs.registeredDistributor ?: client.savedDistributor())
            ?.let { PushDistributor(it, client.label(it)) }

    private fun isLoggedIn() = authStore.getAuthState().value is AuthState.LoggedIn

    private fun publish(status: PushStatus, distributors: List<PushDistributor> = state.value.distributors) {
        mutableState.value = PushState(prefs.enabled, status, distributors)
    }

    private fun RemoteApiResponse.Error.isServerDisabled() =
        this is RemoteApiResponse.Error.Unknown && httpStatus == 503

    companion object {
        const val PUSH_SCOPE = "push"
    }
}
