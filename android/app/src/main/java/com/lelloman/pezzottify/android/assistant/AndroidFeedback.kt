package com.lelloman.pezzottify.android.assistant

import android.content.Context
import com.lelloman.pezzottify.android.domain.auth.*
import com.lelloman.pezzottify.android.domain.remoteapi.*
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.config.BuildInfo
import com.lelloman.pezzottify.android.domain.device.DeviceInfoProvider
import com.lelloman.pezzottify.android.logging.LogFileManager
import com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport.*
import com.lelloman.simpleaiassistant.diagnostics.*
import com.lelloman.simpleaiassistant.data.AccountChatRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import java.util.UUID
import javax.inject.Inject

class AndroidFeedback @Inject constructor(
    @ApplicationContext context:Context,
    private val auth:AuthStore,
    private val recorder:DiagnosticRecorder,
    private val logs:LogFileManager,
    private val api:RemoteApiClient,
    private val build:BuildInfo,
    private val devices:DeviceInfoProvider,
    repository:AccountChatRepository, // Ensure account lifecycle is active even before opening chat.
):BugReportScreenViewModel.Interactor {
    private val preferences=context.getSharedPreferences("assistant_diagnostics",Context.MODE_PRIVATE)
    private fun account()=auth.getAuthState().value as? AuthState.LoggedIn
    private fun identity(state:AuthState.LoggedIn?)=state?.let {"${it.remoteUrl.length}:${it.remoteUrl}${it.userHandle}"}
    override val invalidations:Flow<Any?> = combine(auth.getAuthState().map {identity(it as? AuthState.LoggedIn)}.distinctUntilChanged(),recorder.revision) {owner,revision->owner to revision}.distinctUntilChanged()
    override suspend fun recordingEnabled()=recorder.isEnabled()
    override suspend fun setRecordingEnabled(enabled:Boolean) {
        val owner=checkNotNull(identity(account()))
        withContext(Dispatchers.IO) {check(preferences.edit().putBoolean("enabled:$owner",enabled).commit())}
        check(identity(account())==owner)
        recorder.setAccount(owner,enabled)
    }
    override suspend fun prepare(state:BugReportScreenState,messageId:String?):PreparedFeedback = withContext(Dispatchers.IO) {
        val owner=checkNotNull(identity(account()))
        val revision=recorder.revision.value
        val diagnostic=if(state.includeAssistant) checkNotNull(recorder.snapshot(state.includeAllChats,messageId)) else null
        val attachments=buildList {
            if(state.includeLogs) {
                val redacted=DiagnosticRedactor.text(logs.getLogContent(),Int.MAX_VALUE)
                if(redacted.isNotBlank()) add(FeedbackAttachment("technical_logs",utf8Tail(redacted,1024*1024)))
            }
            diagnostic?.let {add(FeedbackAttachment("assistant",it.content))}
        }
        check(identity(account())==owner && recorder.revision.value==revision)
        val info=devices.getDeviceInfo()
        val request=FeedbackReport(UUID.randomUUID().toString(),state.kind,state.category,state.title.takeIf {it.isNotBlank()},state.description,
            clientVersion=build.versionName,deviceInfo="${info.deviceName ?: info.deviceType} (${info.osInfo ?: "Unknown"})",attachments=attachments)
        PreparedFeedback(request) {
            val current=account()
            if(current==null || identity(current)!=owner || recorder.revision.value!=revision || (diagnostic!=null && !recorder.isCurrent(diagnostic))) {
                FeedbackResult.Failed(401)
            } else {
                currentCoroutineContext().ensureActive()
                check(identity(account())==owner && recorder.revision.value==revision)
                // Both destination and credentials are from one captured account state.
                api.submitFeedback(request,current.remoteUrl,current.authToken)
            }
        }
    }
    override suspend fun list(before:Long?):FeedbackPage {
        val current=checkNotNull(account())
        return when(val result=api.listFeedback(current.remoteUrl,current.authToken,before)) {
            is RemoteApiResponse.Success -> result.data
            is RemoteApiResponse.Error -> error("Reports unavailable")
        }
    }
    companion object {
        internal fun utf8Tail(text:String,limit:Int):String {
            val bytes=text.toByteArray(Charsets.UTF_8)
            if(bytes.size<=limit) return text
            var start=bytes.size-limit
            while(start<bytes.size && (bytes[start].toInt() and 0xc0)==0x80) start++
            return String(bytes,start,bytes.size-start,Charsets.UTF_8)
        }
    }
}
