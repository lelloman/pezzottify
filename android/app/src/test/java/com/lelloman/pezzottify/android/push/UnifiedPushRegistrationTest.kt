package com.lelloman.pezzottify.android.push

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import com.lelloman.pezzottify.android.domain.push.PushDistributor
import com.lelloman.pezzottify.android.domain.push.PushStatus
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.every
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test
import kotlin.reflect.KProperty

@OptIn(ExperimentalCoroutinesApi::class)
class UnifiedPushRegistrationTest {

    private class FakeClient : UnifiedPushClient {
        var installed = mutableListOf<String>()
        var saved: String? = null
        var default: String? = null
        val registrations = mutableListOf<Pair<String?, String>>()
        var unregisterCalls = 0
        override fun distributors() = installed.toList()
        override fun label(packageName: String) = packageName.substringAfterLast('.')
        override fun savedDistributor() = saved
        override fun defaultDistributor() = default
        override fun saveDistributor(packageName: String) { saved = packageName }
        override fun register(vapid: String) { registrations += saved to vapid }
        override fun unregister() { unregisterCalls++ }
    }

    private class FakePrefs : PushPreferences {
        override var enabled = true
        override var endpoint: String? = null
        override var p256dh: String? = null
        override var auth: String? = null
        override var registeredDistributor: String? = null
        override var vapidKey: String? = null
        override var serverEndpoint: String? = null
    }

    private class FakeScheduler : PushWorkScheduler {
        var registrations = 0
        val deletions = mutableListOf<String>()
        var syncs = 0
        override fun scheduleServerRegistration() { registrations++ }
        override fun scheduleServerDeletion(endpoint: String) { deletions += endpoint }
        override fun schedulePushSync() { syncs++ }
    }

    private val client = FakeClient()
    private val prefs = FakePrefs()
    private val scheduler = FakeScheduler()
    private val authState = MutableStateFlow<AuthState>(loggedIn())
    private val remote = mockk<RemoteApiClient>()
    private val notifications = mockk<SystemNotificationHelper>(relaxed = true)
    private val scope = TestScope(UnconfinedTestDispatcher())
    private lateinit var registration: UnifiedPushRegistration

    private fun loggedIn() = AuthState.LoggedIn(userHandle = "u", authToken = "t", remoteUrl = "http://x")

    @Before
    fun setUp() {
        val authStore = mockk<AuthStore> { every { getAuthState() } returns authState }
        val device = mockk<DeviceInfoProvider> {
            every { getDeviceInfo() } returns DeviceInfo(
                deviceUuid = "device-1", deviceType = "android", deviceName = "Phone", osInfo = "36",
            )
        }
        val loggerFactory = mockk<LoggerFactory> {
            every { getValue(any(), any<KProperty<*>>()) } returns mockk<Logger>(relaxed = true)
        }
        coEvery { remote.getPushVapidKey() } returns RemoteApiResponse.Success(VAPID)
        coEvery { remote.putPushRegistration(any(), any(), any(), any()) } returns RemoteApiResponse.Success(Unit)
        coEvery { remote.deletePushRegistration(any()) } returns RemoteApiResponse.Success(Unit)
        registration = UnifiedPushRegistration(
            client, prefs, scheduler, authStore, remote, device, notifications, scope, loggerFactory,
        )
    }

    private fun status() = registration.state.value.status

    @Test
    fun `logged out does not register`() {
        authState.value = AuthState.LoggedOut
        client.installed += NTFY
        registration.initialize()
        assertThat(status()).isEqualTo(PushStatus.LoggedOut)
        assertThat(client.registrations).isEmpty()
    }

    @Test
    fun `no distributor falls back to polling`() {
        registration.initialize()
        assertThat(status()).isEqualTo(PushStatus.NoDistributor)
    }

    @Test
    fun `single distributor registers with the server vapid key and confirms on the server`() = runTest {
        client.installed += NTFY
        registration.initialize()
        assertThat(client.registrations).containsExactly(NTFY to VAPID)
        assertThat(status()).isEqualTo(PushStatus.Registering(PushDistributor(NTFY, "ntfy")))

        registration.onNewEndpoint("https://push.example/e1", "p256", "auth")
        assertThat(scheduler.registrations).isEqualTo(1)

        assertThat(registration.registerOnServer()).isEqualTo(ServerSyncResult.Done)
        coVerify { remote.putPushRegistration("https://push.example/e1", "p256", "auth", "device-1") }
        assertThat(prefs.serverEndpoint).isEqualTo("https://push.example/e1")
        assertThat(status()).isEqualTo(PushStatus.Registered(PushDistributor(NTFY, "ntfy")))

        // A later refresh keeps the working registration without registering again.
        registration.refresh()
        assertThat(client.registrations).hasSize(1)
        assertThat(status()).isInstanceOf(PushStatus.Registered::class.java)
    }

    @Test
    fun `several distributors without a default ask the user to choose`() {
        client.installed += listOf(NTFY, STORE)
        registration.initialize()
        assertThat(status()).isEqualTo(PushStatus.ChooseDistributor)
        assertThat(registration.state.value.distributors.map { it.packageName })
            .containsExactly(NTFY, STORE)

        registration.chooseDistributor(STORE)
        assertThat(client.registrations).containsExactly(STORE to VAPID)
    }

    @Test
    fun `server with push disabled is reported and nothing is registered`() {
        coEvery { remote.getPushVapidKey() } returns RemoteApiResponse.Error.Unknown("off", 503)
        client.installed += NTFY
        registration.initialize()
        assertThat(status()).isEqualTo(PushStatus.ServerUnsupported)
        assertThat(client.registrations).isEmpty()
    }

    @Test
    fun `a new endpoint replaces the old one on the server`() {
        client.installed += NTFY
        registration.initialize()
        prefs.endpoint = "https://push.example/old"
        prefs.serverEndpoint = "https://push.example/old"
        registration.onNewEndpoint("https://push.example/new", "p", "a")
        assertThat(scheduler.deletions).containsExactly("https://push.example/old")
        assertThat(prefs.serverEndpoint).isNull()
        assertThat(scheduler.registrations).isEqualTo(1)
    }

    @Test
    fun `an endpoint without encryption keys is rejected`() {
        client.installed += NTFY
        registration.initialize()
        registration.onNewEndpoint("https://push.example/e1", null, null)
        assertThat(status()).isInstanceOf(PushStatus.Failed::class.java)
        assertThat(scheduler.registrations).isEqualTo(0)
    }

    @Test
    fun `disabling unregisters locally and on the server`() {
        client.installed += NTFY
        registration.initialize()
        registration.onNewEndpoint("https://push.example/e1", "p", "a")
        prefs.serverEndpoint = "https://push.example/e1"

        registration.setEnabled(false)
        assertThat(client.unregisterCalls).isEqualTo(1)
        assertThat(scheduler.deletions).containsExactly("https://push.example/e1")
        assertThat(prefs.endpoint).isNull()
        assertThat(status()).isEqualTo(PushStatus.Disabled)
        assertThat(registration.state.value.enabled).isFalse()
    }

    @Test
    fun `logout deletes the registration while the credential is valid`() = runTest {
        client.installed += NTFY
        registration.initialize()
        registration.onNewEndpoint("https://push.example/e1", "p", "a")
        prefs.serverEndpoint = "https://push.example/e1"

        registration.unregisterForLogout()
        coVerify { remote.deletePushRegistration("https://push.example/e1") }
        assertThat(client.unregisterCalls).isEqualTo(1)
        assertThat(prefs.endpoint).isNull()
        assertThat(status()).isEqualTo(PushStatus.LoggedOut)
    }

    @Test
    fun `a push message schedules a sync`() {
        registration.onMessage("""{"type":"sync","seq":3}""".toByteArray())
        registration.onMessage("not json".toByteArray())
        registration.onMessage(ByteArray(0))
        assertThat(scheduler.syncs).isEqualTo(3)
        verify(exactly = 0) { notifications.showTestPushNotification(any(), any()) }
    }

    @Test
    fun `a test message shows a notification instead of syncing`() {
        registration.onMessage(
            """{"type":"test","title":" Hello ","body":"From admin","sent_at":1}""".toByteArray()
        )
        verify { notifications.showTestPushNotification("Hello", "From admin") }
        assertThat(scheduler.syncs).isEqualTo(0)
    }

    @Test
    fun `test payloads fall back to defaults and are capped`() {
        assertThat(PushPayload.parse("""{"type":"test"}""".toByteArray()))
            .isEqualTo(PushPayload.Test(PushPayload.DEFAULT_TEST_TITLE, ""))
        val long = PushPayload.parse(
            """{"type":"test","title":"${"t".repeat(150)}","body":"${"b".repeat(400)}"}""".toByteArray()
        ) as PushPayload.Test
        assertThat(long.title.length).isEqualTo(100)
        assertThat(long.body.length).isEqualTo(300)
        assertThat(PushPayload.parse("""{"type":"test","title":{"x":1}}""".toByteArray()))
            .isEqualTo(PushPayload.Test(PushPayload.DEFAULT_TEST_TITLE, ""))
        assertThat(PushPayload.parse("[1]".toByteArray())).isEqualTo(PushPayload.Wake)
    }

    @Test
    fun `server errors are retried except when push is disabled`() = runTest {
        client.installed += NTFY
        registration.initialize()
        registration.onNewEndpoint("https://push.example/e1", "p", "a")

        coEvery { remote.putPushRegistration(any(), any(), any(), any()) } returns RemoteApiResponse.Error.Network
        assertThat(registration.registerOnServer()).isEqualTo(ServerSyncResult.Retry)

        coEvery { remote.putPushRegistration(any(), any(), any(), any()) } returns
            RemoteApiResponse.Error.Unknown("off", 503)
        assertThat(registration.registerOnServer()).isEqualTo(ServerSyncResult.Done)
        assertThat(status()).isEqualTo(PushStatus.ServerUnsupported)
    }

    @Test
    fun `unregistered by the distributor clears state without re-registering`() {
        client.installed += NTFY
        registration.initialize()
        registration.onNewEndpoint("https://push.example/e1", "p", "a")
        registration.onUnregistered()
        assertThat(prefs.endpoint).isNull()
        assertThat(scheduler.deletions).containsExactly("https://push.example/e1")
        assertThat(status()).isInstanceOf(PushStatus.Failed::class.java)
        assertThat(client.registrations).hasSize(1)
    }

    private companion object {
        const val NTFY = "io.heckel.ntfy"
        const val STORE = "com.lelloman.store"
        const val VAPID = "BAx"
    }
}
