package com.lelloman.pezzottify.android.ui.screen.main.content.work

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.*
import com.lelloman.pezzottify.android.domain.statics.*
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWork
import io.mockk.*
import kotlinx.coroutines.*
import kotlinx.coroutines.test.*
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class WorkScreenViewModelTest {
    private val dispatcher = StandardTestDispatcher()
    private val api = mockk<RemoteApiClient>()
    private val work = Work("w", "Work")
    @Before fun setup() { Dispatchers.setMain(dispatcher) }
    @After fun teardown() { Dispatchers.resetMain() }
    private fun vm() = WorkScreenViewModel(GetWork(api), mockk(relaxed = true), mockk(), "w")

    @Test fun `late scope response cannot replace the selected scope`() = runTest(dispatcher) {
        coEvery { api.getWork("w", 50, 0, "all") } coAnswers {
            withContext(NonCancellable) { delay(1000) }
            RemoteApiResponse.Success(WorkPage(work.copy(title = "stale"), nextOffset = 50, hasMore = true))
        }
        coEvery { api.getWork("w", 50, 0, "parts") } returns RemoteApiResponse.Success(WorkPage(work, nextOffset = 0, hasMore = false))
        val vm = vm()
        runCurrent()
        vm.changeScope(WorkScope.Parts)
        advanceUntilIdle()
        assertThat(vm.state.value.scope).isEqualTo(WorkScope.Parts)
        assertThat(vm.state.value.work?.title).isEqualTo("Work")
        assertThat(vm.state.value.hasMore).isFalse()
    }

    @Test fun `page failure keeps work and retries same cursor`() = runTest(dispatcher) {
        coEvery { api.getWork("w", 50, 0, "all") } returns RemoteApiResponse.Success(WorkPage(work, nextOffset = 50, hasMore = true))
        coEvery { api.getWork("w", 50, 50, "all") } returns RemoteApiResponse.Error.Network
        val vm = vm()
        advanceUntilIdle()
        vm.loadMore()
        advanceUntilIdle()
        assertThat(vm.state.value.failed).isTrue()
        assertThat(vm.state.value.work).isEqualTo(work)
        coEvery { api.getWork("w", 50, 50, "all") } returns RemoteApiResponse.Success(WorkPage(work, nextOffset = 50, hasMore = false))
        vm.loadMore()
        advanceUntilIdle()
        assertThat(vm.state.value.failed).isFalse()
        assertThat(vm.state.value.hasMore).isFalse()
    }

    @Test fun `missing work is distinct from network failure`() = runTest(dispatcher) {
        coEvery { api.getWork(any(), any(), any(), any()) } returns RemoteApiResponse.Error.NotFound
        val vm = vm()
        advanceUntilIdle()
        assertThat(vm.state.value.notFound).isTrue()
    }
}
