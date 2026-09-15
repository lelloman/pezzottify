package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.domain.remoteapi.*
import com.lelloman.pezzottify.android.ui.R
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.*
import javax.inject.Inject

@HiltViewModel
class BugReportScreenViewModel @Inject constructor(private val interactor:Interactor):ViewModel(),BugReportScreenActions {
    private val mutableState=MutableStateFlow(BugReportScreenState())
    val state:StateFlow<BugReportScreenState> = mutableState.asStateFlow()
    private var job:Job?=null
    private var reportsJob:Job?=null
    private var messageId:String?=null
    init {
        viewModelScope.launch {
            interactor.invalidations.collect {
                job?.cancel();reportsJob?.cancel()
                mutableState.value=BugReportScreenState(recordingEnabled=interactor.recordingEnabled(),category=if(messageId!=null) "assistant" else "other")
                refreshReports()
            }
        }
    }
    fun selectMessage(id:String?) {messageId=id;if(id!=null) onCategoryChanged("assistant")}
    private fun edit(change:(BugReportScreenState)->BugReportScreenState) {
        if(state.value.isSubmitting) return
        mutableState.update {change(it).copy(preview=null,reportId=null,submitResult=null,errorRes=null,retryAfterSeconds=null)}
    }
    override fun onTitleChanged(title:String)=edit {it.copy(title=title)}
    override fun onDescriptionChanged(description:String)=edit {it.copy(description=description)}
    override fun onIncludeLogsChanged(includeLogs:Boolean)=edit {it.copy(includeLogs=includeLogs)}
    override fun onKindChanged(kind:String)=edit {it.copy(kind=kind)}
    override fun onCategoryChanged(category:String)=edit {it.copy(category=category)}
    override fun onIncludeAssistantChanged(include:Boolean)=edit {it.copy(includeAssistant=include)}
    override fun onIncludeAllChatsChanged(include:Boolean)=edit {it.copy(includeAllChats=include)}
    override fun onRecordingChanged(enabled:Boolean) {
        if(state.value.isSubmitting) return
        job=viewModelScope.launch {
            try {interactor.setRecordingEnabled(enabled)}
            catch(e:CancellationException) {throw e}
            catch(_:Exception) {mutableState.update {it.copy(errorRes=R.string.feedback_prepare_failed)}}
        }
    }
    override fun submit() {
        val current=state.value
        if(current.isSubmitting || current.submitResult is SubmitResult.Success) return
        if(current.description.isBlank()) {mutableState.update {it.copy(errorRes=R.string.bug_report_description_required)};return}
        if(current.title.toByteArray().size>200 || current.description.toByteArray().size>100*1024) {mutableState.update {it.copy(errorRes=R.string.feedback_too_large)};return}
        mutableState.update {it.copy(isSubmitting=true,errorRes=null,submitResult=null,retryAfterSeconds=null)}
        job=viewModelScope.launch {
            try {
                if(current.preview==null) {
                    val preview=interactor.prepare(current,messageId)
                    mutableState.update {it.copy(preview=preview,isSubmitting=false)}
                } else {
                    when(val result=current.preview.send()) {
                        is FeedbackResult.Sent -> {mutableState.update {it.copy(isSubmitting=false,submitResult=SubmitResult.Success,reportId=result.id,preview=null)};refreshReports()}
                        is FeedbackResult.Failed -> mutableState.update {it.copy(isSubmitting=false,retryAfterSeconds=result.retryAfterSeconds,errorRes=when(result.status) {
                            401,403 -> R.string.feedback_sign_in
                            409 -> R.string.feedback_conflict
                            413 -> R.string.feedback_too_large
                            429 -> R.string.feedback_rate_limit
                            503 -> R.string.feedback_capacity
                            else -> R.string.feedback_send_failed
                        })}
                    }
                }
            } catch(e:CancellationException) {throw e}
            catch(_:Exception) {mutableState.update {it.copy(isSubmitting=false,errorRes=R.string.feedback_prepare_failed)}}
        }
    }
    override fun refreshReports(loadMore:Boolean) {
        if(state.value.reportsLoading) return
        val before=if(loadMore) state.value.nextCursor ?: return else null
        mutableState.update {it.copy(reportsLoading=true,reportsError=false)}
        reportsJob=viewModelScope.launch {
            try {
                val page=interactor.list(before)
                mutableState.update {it.copy(reportsLoading=false,reports=if(loadMore) it.reports+page.items else page.items,nextCursor=page.nextCursor)}
            } catch(e:CancellationException) {throw e}
            catch(_:Exception) {mutableState.update {it.copy(reportsLoading=false,reportsError=true)}}
        }
    }
    interface Interactor {
        val invalidations:Flow<Any?>
        suspend fun recordingEnabled():Boolean
        suspend fun setRecordingEnabled(enabled:Boolean)
        suspend fun prepare(state:BugReportScreenState,messageId:String?):PreparedFeedback
        suspend fun list(before:Long?):FeedbackPage
    }
}
