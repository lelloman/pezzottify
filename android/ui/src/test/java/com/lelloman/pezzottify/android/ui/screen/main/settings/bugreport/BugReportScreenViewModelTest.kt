package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.ui.R
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class BugReportScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()
    private lateinit var fakeInteractor: FakeInteractor
    private lateinit var viewModel: BugReportScreenViewModel

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        fakeInteractor = FakeInteractor()
        viewModel = BugReportScreenViewModel(fakeInteractor)
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    @Test
    fun `initial state has empty title and description`() {
        assertThat(viewModel.state.value.title).isEmpty()
        assertThat(viewModel.state.value.description).isEmpty()
        assertThat(viewModel.state.value.includeLogs).isTrue()
        assertThat(viewModel.state.value.isSubmitting).isFalse()
        assertThat(viewModel.state.value.submitResult).isNull()
    }

    @Test
    fun `onTitleChanged updates title`() {
        viewModel.onTitleChanged("Bug title")

        assertThat(viewModel.state.value.title).isEqualTo("Bug title")
    }

    @Test
    fun `onDescriptionChanged updates description`() {
        viewModel.onDescriptionChanged("Bug description")

        assertThat(viewModel.state.value.description).isEqualTo("Bug description")
    }

    @Test
    fun `onIncludeLogsChanged updates includeLogs`() {
        viewModel.onIncludeLogsChanged(false)

        assertThat(viewModel.state.value.includeLogs).isFalse()
    }

    @Test
    fun `submit with empty description sets error`() = runTest {
        viewModel.submit()
        advanceUntilIdle()

        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.bug_report_description_required)
        assertThat(viewModel.state.value.isSubmitting).isFalse()
        assertThat(fakeInteractor.submitCalled).isFalse()
    }

    @Test
    fun `submit with blank description sets error`() = runTest {
        viewModel.onDescriptionChanged("   ")
        viewModel.submit()
        advanceUntilIdle()

        assertThat(viewModel.state.value.errorRes).isEqualTo(R.string.bug_report_description_required)
    }

    @Test
    fun `submit with valid description calls interactor`() = runTest {
        viewModel.onTitleChanged("Test title")
        viewModel.onDescriptionChanged("Test description")
        viewModel.submit()
        advanceUntilIdle()

        assertThat(fakeInteractor.submitCalled).isTrue()
        assertThat(fakeInteractor.lastTitle).isEqualTo("Test title")
        assertThat(fakeInteractor.lastDescription).isEqualTo("Test description")
    }

    @Test
    fun `submit with empty title passes null to interactor`() = runTest {
        viewModel.onDescriptionChanged("Test description")
        viewModel.submit()
        advanceUntilIdle()

        assertThat(fakeInteractor.lastTitle).isNull()
    }

    @Test
    fun `submit includes logs when includeLogs is true`() = runTest {
        fakeInteractor.logsToReturn = "Some logs"
        viewModel.onDescriptionChanged("Test description")
        viewModel.onIncludeLogsChanged(true)
        viewModel.submit()
        advanceUntilIdle()

        assertThat(fakeInteractor.lastLogs).isEqualTo("Some logs")
    }

    @Test
    fun `submit does not include logs when includeLogs is false`() = runTest {
        fakeInteractor.logsToReturn = "Some logs"
        viewModel.onDescriptionChanged("Test description")
        viewModel.onIncludeLogsChanged(false)
        viewModel.submit()
        advanceUntilIdle()

        assertThat(fakeInteractor.lastLogs).isNull()
    }

    @Test
    fun `successful submit shows success result`() = runTest {
        fakeInteractor.resultToReturn = SubmitResult.Success
        viewModel.onDescriptionChanged("Test description")
        viewModel.submit()
        advanceUntilIdle()

        assertThat(viewModel.state.value.submitResult).isEqualTo(SubmitResult.Success)
        assertThat(viewModel.state.value.isSubmitting).isFalse()
    }

    @Test
    fun `failed submit shows error result`() = runTest {
        fakeInteractor.resultToReturn = SubmitResult.Error("Network error")
        viewModel.onDescriptionChanged("Test description")
        viewModel.submit()
        advanceUntilIdle()

        assertThat(viewModel.state.value.submitResult).isEqualTo(SubmitResult.Error("Network error"))
        assertThat(viewModel.state.value.isSubmitting).isFalse()
    }

    @Test
    fun `changing input clears previous error and result`() = runTest {
        // First submit fails
        fakeInteractor.resultToReturn = SubmitResult.Error("Error")
        viewModel.onDescriptionChanged("Test")
        viewModel.submit()
        advanceUntilIdle()

        assertThat(viewModel.state.value.submitResult).isNotNull()

        // Changing description clears result
        viewModel.onDescriptionChanged("New description")

        assertThat(viewModel.state.value.submitResult).isNull()
        assertThat(viewModel.state.value.errorRes).isNull()
    }

    private class FakeInteractor : BugReportScreenViewModel.Interactor {
        var logsToReturn: String? = null
        var resultToReturn: SubmitResult = SubmitResult.Success

        var submitCalled = false
        var lastTitle: String? = null
        var lastDescription: String? = null
        var lastLogs: String? = null

        override fun getLogs(): String? = logsToReturn

        override suspend fun submitBugReport(
            title: String?,
            description: String,
            logs: String?,
        ): SubmitResult {
            submitCalled = true
            lastTitle = title
            lastDescription = description
            lastLogs = logs
            return resultToReturn
        }
    }
}
