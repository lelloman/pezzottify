package com.lelloman.pezzottify.android.assistant

import android.content.Context
import com.lelloman.pezzottify.android.domain.auth.*
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.domain.remoteapi.*
import com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport.BugReportScreenState
import com.lelloman.simpleaiassistant.diagnostics.*
import io.mockk.*
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class FeedbackAccountIsolationTest {
    @Test fun `cleared or foreign account snapshots cannot be uploaded`() = runBlocking {
        val context=mockk<Context>()
        every {context.getSharedPreferences(any(),any())} returns mockk(relaxed=true)
        val accounts=MutableStateFlow<AuthState>(AuthState.LoggedIn("alice","token-a",remoteUrl="https://one.example/"))
        val auth=mockk<AuthStore>();every {auth.getAuthState()} returns accounts
        val build=mockk<BuildInfo>();every {build.versionName} returns "test"
        val device=mockk<DeviceInfoProvider>();every {device.getDeviceInfo()} returns DeviceInfo("id","phone","Test","Android")
        val recorder=DiagnosticRecorder(object:DiagnosticStorage {
            override fun read(maxBytes:Int):String?=null
            override fun write(content:String) {}
            override fun clear() {}
        })
        recorder.setAccount("alice",true)
        val api=mockk<RemoteApiClient>()
        val feedback=AndroidFeedback(context,auth,recorder,mockk(),api,build,device,mockk())
        recorder.begin("private chat")
        val first=feedback.prepare(BugReportScreenState(description="report",includeAssistant=true),null)
        recorder.clear()
        assertEquals(401,(first.send() as FeedbackResult.Failed).status)
        recorder.begin("another chat")
        val second=feedback.prepare(BugReportScreenState(description="report",includeAssistant=true),null)
        accounts.value=AuthState.LoggedIn("bob","token-b",remoteUrl="https://two.example/")
        assertEquals(401,(second.send() as FeedbackResult.Failed).status)
        coVerify(exactly=0) {api.submitFeedback(any(),any(),any())}
    }
}
