package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.*
import com.lelloman.pezzottify.android.ui.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.*
import org.junit.*

@OptIn(ExperimentalCoroutinesApi::class)
class BugReportScreenViewModelTest {
    private val dispatcher=StandardTestDispatcher()
    @Before fun before(){Dispatchers.setMain(dispatcher)}
    @After fun after(){Dispatchers.resetMain()}
    private class Fake:BugReportScreenViewModel.Interactor {
        override val invalidations=MutableStateFlow<Any?>(0)
        var recorded=false
        var prepared=0
        val sent=mutableListOf<FeedbackReport>()
        var result:FeedbackResult=FeedbackResult.Failed(429)
        override suspend fun recordingEnabled()=recorded
        override suspend fun setRecordingEnabled(enabled:Boolean) {recorded=enabled;invalidations.value=System.nanoTime()}
        override suspend fun list(before:Long?)=FeedbackPage(emptyList())
        override suspend fun prepare(state:BugReportScreenState,messageId:String?):PreparedFeedback {
            prepared++
            val request=FeedbackReport("request-$prepared",state.kind,state.category,state.title,state.description,clientVersion="test",deviceInfo=null,
                attachments=if(state.includeAssistant) listOf(FeedbackAttachment("assistant","fixed snapshot")) else emptyList())
            return PreparedFeedback(request) {sent+=request;result}
        }
    }
    @Test fun `chat is opt in and review does not upload`() = runTest {
        val fake=Fake();val vm=BugReportScreenViewModel(fake);advanceUntilIdle()
        assertThat(vm.state.value.includeAssistant).isFalse()
        assertThat(vm.state.value.recordingEnabled).isFalse()
        vm.onDescriptionChanged("description");vm.submit();advanceUntilIdle()
        assertThat(vm.state.value.preview).isNotNull()
        assertThat(fake.sent).isEmpty()
    }
    @Test fun `manual retry reuses exact snapshot and editing invalidates it`() = runTest {
        val fake=Fake();val vm=BugReportScreenViewModel(fake);advanceUntilIdle()
        vm.onDescriptionChanged("description");vm.onIncludeAssistantChanged(true);vm.submit();advanceUntilIdle()
        val snapshot=vm.state.value.preview!!.request
        vm.submit();advanceUntilIdle();vm.submit();advanceUntilIdle()
        assertThat(fake.sent).containsExactly(snapshot,snapshot)
        assertThat(fake.prepared).isEqualTo(1)
        assertThat(vm.state.value.errorRes).isEqualTo(R.string.feedback_rate_limit)
        vm.onDescriptionChanged("edited")
        assertThat(vm.state.value.preview).isNull()
        vm.submit();advanceUntilIdle()
        assertThat(vm.state.value.preview!!.request.clientRequestId).isNotEqualTo(snapshot.clientRequestId)
    }
    @Test fun `account or recorder invalidation clears draft and preview`() = runTest {
        val fake=Fake();val vm=BugReportScreenViewModel(fake);advanceUntilIdle()
        vm.onDescriptionChanged("private");vm.submit();advanceUntilIdle()
        fake.invalidations.value=1;advanceUntilIdle()
        assertThat(vm.state.value.preview).isNull()
        assertThat(vm.state.value.description).isEmpty()
        vm.submit();advanceUntilIdle();assertThat(fake.sent).isEmpty()
    }
    @Test fun `byte validation and success receipt`() = runTest {
        val fake=Fake();val vm=BugReportScreenViewModel(fake);advanceUntilIdle()
        vm.onDescriptionChanged("valid");vm.onTitleChanged("🌍".repeat(51));vm.submit();advanceUntilIdle()
        assertThat(vm.state.value.errorRes).isEqualTo(R.string.feedback_too_large)
        vm.onTitleChanged("ok");vm.submit();advanceUntilIdle()
        fake.result=FeedbackResult.Sent("report-id");vm.submit();advanceUntilIdle()
        assertThat(vm.state.value.reportId).isEqualTo("report-id")
        assertThat(vm.state.value.preview).isNull()
        assertThat(vm.state.value.submitResult).isEqualTo(SubmitResult.Success)
    }
}
