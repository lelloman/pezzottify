package com.lelloman.pezzottify.android.oidc

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager.PollResult
import com.lelloman.pezzottify.android.domain.auth.oidc.DeviceFlowAuthManager.StartResult
import com.lelloman.pezzottify.android.domain.auth.oidc.OidcConfig
import com.lelloman.pezzottify.android.domain.remoteapi.DeviceInfo
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.test.runTest
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Protocol
import okhttp3.Request
import okhttp3.Response
import okhttp3.ResponseBody.Companion.toResponseBody
import okio.Buffer
import org.junit.Before
import org.junit.Test
import java.io.IOException
import java.util.Base64
import kotlin.reflect.KProperty

class OkHttpDeviceFlowAuthManagerTest {

    private val requests = mutableListOf<Request>()
    private var respond: (Request) -> Pair<Int, String> = { 500 to "" }
    private var endpoints: DeviceFlowEndpointsProvider.Endpoints? = DeviceFlowEndpointsProvider.Endpoints(
        deviceAuthorizationEndpoint = "https://auth.example.com/device_authorization",
        tokenEndpoint = "https://auth.example.com/token",
    )
    private lateinit var manager: OkHttpDeviceFlowAuthManager
    private val deviceInfo = DeviceInfo(
        deviceUuid = "device-uuid",
        deviceType = "android_tv",
        deviceName = "Living room TV",
        osInfo = "Android 11",
    )

    @Before
    fun setUp() {
        val client = OkHttpClient.Builder()
            .addInterceptor { chain ->
                val request = chain.request()
                requests += request
                val (code, body) = respond(request)
                if (code == -1) throw IOException("offline")
                Response.Builder()
                    .request(request)
                    .protocol(Protocol.HTTP_1_1)
                    .code(code)
                    .message("")
                    .body(body.toResponseBody("application/json".toMediaType()))
                    .build()
            }
            .build()
        val loggerFactory = mockk<LoggerFactory>()
        every { loggerFactory.getValue(any(), any<KProperty<*>>()) } returns mockk<Logger>(relaxed = true)
        manager = OkHttpDeviceFlowAuthManager(
            httpClient = client,
            endpointsProvider = object : DeviceFlowEndpointsProvider {
                override suspend fun deviceFlowEndpoints() = endpoints
            },
            oidcConfig = OidcConfig(
                issuerUrl = "https://auth.example.com",
                clientId = "pezzottify-android",
                redirectUri = "app://oauth/callback",
            ),
            loggerFactory = loggerFactory,
        )
    }

    @Test
    fun `start posts client id and scopes and parses the authorization`() = runTest {
        respond = {
            200 to """{"device_code":"dc_1","user_code":"WDJB-MJHT",
                "verification_uri":"https://auth.example.com/device",
                "verification_uri_complete":"https://auth.example.com/device?user_code=WDJB-MJHT",
                "expires_in":600,"interval":5}"""
        }

        val result = manager.start(deviceInfo) as StartResult.Success

        val request = requests.single()
        assertThat(request.url.toString()).isEqualTo("https://auth.example.com/device_authorization")
        assertThat(request.method).isEqualTo("POST")
        assertThat(request.formBody()).isEqualTo(
            "client_id=pezzottify-android&scope=openid%20profile%20email" +
                "&device_id=device-uuid&device_type=android_tv&device_name=Living%20room%20TV"
        )
        with(result.authorization) {
            assertThat(deviceCode).isEqualTo("dc_1")
            assertThat(userCode).isEqualTo("WDJB-MJHT")
            assertThat(verificationUri).isEqualTo("https://auth.example.com/device")
            assertThat(verificationUriComplete).isEqualTo("https://auth.example.com/device?user_code=WDJB-MJHT")
            assertThat(qrCodeUrl).isNull()
            assertThat(expiresInMs).isEqualTo(600_000)
            assertThat(intervalMs).isEqualTo(5_000)
        }
    }

    @Test
    fun `start omits device_name when the device has none`() = runTest {
        respond = { 400 to """{"error":"unauthorized_client"}""" }

        manager.start(deviceInfo.copy(deviceName = null))

        val form = requests.single().formBody()
        assertThat(form).contains("device_id=device-uuid")
        assertThat(form).contains("device_type=android_tv")
        assertThat(form).doesNotContain("device_name")
    }

    @Test
    fun `start keeps the optional qr code url and defaults the interval`() = runTest {
        respond = {
            200 to """{"device_code":"dc_1","user_code":"A","verification_uri":"https://a/device",
                "qr_code_url":"lelloauth://authorize?device_code=dc_1","expires_in":300}"""
        }

        val authorization = (manager.start(deviceInfo) as StartResult.Success).authorization

        assertThat(authorization.qrCodeUrl).isEqualTo("lelloauth://authorize?device_code=dc_1")
        assertThat(authorization.verificationUriComplete).isNull()
        assertThat(authorization.intervalMs).isEqualTo(5_000)
        assertThat(authorization.expiresInMs).isEqualTo(300_000)
    }

    @Test
    fun `start reports the server error description`() = runTest {
        respond = { 400 to """{"error":"unauthorized_client","error_description":"Device flow is not enabled"}""" }

        assertThat(manager.start(deviceInfo)).isEqualTo(StartResult.Error("Device flow is not enabled"))
    }

    @Test
    fun `start fails without a device authorization endpoint`() = runTest {
        endpoints = endpoints!!.copy(deviceAuthorizationEndpoint = null)

        assertThat(manager.start(deviceInfo)).isInstanceOf(StartResult.Error::class.java)
        assertThat(requests).isEmpty()
    }

    @Test
    fun `start fails when discovery is unavailable`() = runTest {
        endpoints = null

        assertThat(manager.start(deviceInfo)).isInstanceOf(StartResult.Error::class.java)
    }

    @Test
    fun `poll posts the device code grant`() = runTest {
        respond = { 400 to """{"error":"authorization_pending"}""" }

        assertThat(manager.poll("dc_1")).isEqualTo(PollResult.Pending)

        val request = requests.single()
        assertThat(request.url.toString()).isEqualTo("https://auth.example.com/token")
        assertThat(request.formBody()).isEqualTo(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code" +
                "&device_code=dc_1&client_id=pezzottify-android"
        )
    }

    @Test
    fun `poll maps the RFC 8628 error codes`() = runTest {
        fun errorBody(error: String) = """{"error":"$error"}"""

        respond = { 400 to errorBody("slow_down") }
        assertThat(manager.poll("dc")).isEqualTo(PollResult.SlowDown)
        respond = { 400 to errorBody("expired_token") }
        assertThat(manager.poll("dc")).isEqualTo(PollResult.Expired)
        respond = { 400 to errorBody("access_denied") }
        assertThat(manager.poll("dc")).isEqualTo(PollResult.Denied)
        respond = { 400 to errorBody("invalid_grant") }
        assertThat(manager.poll("dc")).isInstanceOf(PollResult.Error::class.java)
        respond = { 429 to "" }
        assertThat(manager.poll("dc")).isEqualTo(PollResult.SlowDown)
    }

    @Test
    fun `poll returns network failures as errors`() = runTest {
        respond = { -1 to "" }

        assertThat(manager.poll("dc")).isInstanceOf(PollResult.Error::class.java)
    }

    @Test
    fun `approved poll returns the tokens and the user handle from the id token`() = runTest {
        val idToken = jwt("""{"sub":"42","email":"user@example.com"}""")
        respond = { 200 to """{"access_token":"at","id_token":"$idToken","refresh_token":"rt","token_type":"Bearer"}""" }

        assertThat(manager.poll("dc")).isEqualTo(
            PollResult.Approved(idToken = idToken, refreshToken = "rt", userHandle = "user@example.com")
        )
    }

    @Test
    fun `approved poll without id token is an error`() = runTest {
        respond = { 200 to """{"access_token":"at","token_type":"Bearer"}""" }

        assertThat(manager.poll("dc")).isInstanceOf(PollResult.Error::class.java)
    }

    private fun Request.formBody(): String = Buffer().also { body!!.writeTo(it) }.readUtf8()

    private fun jwt(payload: String): String {
        val encoder = Base64.getUrlEncoder().withoutPadding()
        return listOf("""{"alg":"RS256"}""", payload)
            .joinToString(".") { encoder.encodeToString(it.toByteArray()) } + ".signature"
    }
}
