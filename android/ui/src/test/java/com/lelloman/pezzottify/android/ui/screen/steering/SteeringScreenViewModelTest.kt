package com.lelloman.pezzottify.android.ui.screen.steering

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.player.Gravity
import com.lelloman.pezzottify.android.domain.player.GravityCriterion
import com.lelloman.pezzottify.android.domain.player.GravityDiagnostics
import com.lelloman.pezzottify.android.domain.player.GravityKnobs
import com.lelloman.pezzottify.android.domain.player.GravityNamespaceDiagnostics
import com.lelloman.pezzottify.android.domain.player.GravityReference
import com.lelloman.pezzottify.android.domain.player.GravitySource
import com.lelloman.pezzottify.android.domain.player.PlaybackGravityState
import com.lelloman.pezzottify.android.domain.remoteapi.response.RadioCriterionOption
import com.lelloman.pezzottify.android.domain.remoteapi.response.RadioOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class SteeringScreenViewModelTest {

    private val testDispatcher = StandardTestDispatcher()
    private lateinit var interactor: FakeInteractor

    @Before
    fun setUp() {
        Dispatchers.setMain(testDispatcher)
        interactor = FakeInteractor()
    }

    @After
    fun tearDown() {
        Dispatchers.resetMain()
    }

    private fun queue(gravity: Gravity, trackIds: List<String> = listOf("t1", "t2", "s1")) =
        PlaybackGravityState(gravity = gravity, trackIds = trackIds, hasPlaylist = true)

    private val artist = GravityReference("artist", "a1", "Artist One")

    @Test
    fun `empty states for nothing playing and radio`() = runTest {
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()
        assertThat(viewModel.state.value.availability).isEqualTo(SteeringAvailability.NothingPlaying)

        interactor.state.value = PlaybackGravityState(hasPlaylist = true, isRadio = true)
        advanceUntilIdle()
        assertThat(viewModel.state.value.availability).isEqualTo(SteeringAvailability.Radio)
    }

    @Test
    fun `queue source counts user chosen and suggested tracks`() = runTest {
        interactor.state.value = queue(Gravity(autoTrackIds = listOf("s1", "removed")))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        val state = viewModel.state.value
        assertThat(state.availability).isEqualTo(SteeringAvailability.Ready)
        assertThat(state.source).isEqualTo(SteeringSource.Queue(userChosenCount = 2, suggestedCount = 1))
        assertThat(state.destination).isNull()
    }

    @Test
    fun `destination shows progress, remaining steps and similarity`() = runTest {
        val diagnostics = GravityDiagnostics(
            namespaces = listOf(GravityNamespaceDiagnostics("musicfm.mean.v1", queryToDestination = 0.42)),
        )
        interactor.state.value = queue(
            Gravity(destination = artist, stepsTotal = 10, stepsDone = 4, lastDiagnostics = diagnostics),
        )
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        val state = viewModel.state.value
        assertThat(state.destination?.reference?.label).isEqualTo("Artist One")
        assertThat(state.destination?.progress).isWithin(1e-6f).of(0.4f)
        assertThat(state.destination?.queryToDestination).isWithin(1e-6f).of(0.42f)
        assertThat(state.stepsRemaining).isEqualTo(6)
        assertThat(state.canUseExplore).isFalse()
    }

    @Test
    fun `editing remaining steps keeps steps done and can arrive`() = runTest {
        interactor.state.value = queue(Gravity(destination = artist, stepsTotal = 10, stepsDone = 4))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.setStepsRemaining(3)
        assertThat(interactor.gravity.stepsTotal).isEqualTo(7)
        assertThat(interactor.gravity.stepsDone).isEqualTo(4)
        assertThat(interactor.gravity.destination).isEqualTo(artist)

        viewModel.setStepsRemaining(0)
        // Remaining is clamped to one step, so the destination is still ahead.
        assertThat(interactor.gravity.stepsTotal).isEqualTo(5)
    }

    @Test
    fun `clear destination keeps the source`() = runTest {
        interactor.state.value = queue(Gravity(destination = artist))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.clearDestination()
        assertThat(interactor.gravity.destination).isNull()
        assertThat(interactor.gravity.source.kind).isEqualTo(GravitySource.KIND_QUEUE)
    }

    @Test
    fun `knob edits write gravity knobs and reset clears them`() = runTest {
        interactor.state.value = queue(Gravity())
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()
        assertThat(viewModel.state.value.knobs.isDefault).isTrue()
        assertThat(viewModel.state.value.knobs.recencyWeight).isEqualTo(SteeringKnobs.DEFAULT_RECENCY_WEIGHT)

        viewModel.setRecencyWeight(0.5f)
        viewModel.setDiversity(0.7f)
        viewModel.setMode(SteeringKnobs.MODE_EXPLORE)
        advanceUntilIdle()
        assertThat(interactor.gravity.knobs.recencyWeight).isWithin(1e-6).of(0.5)
        assertThat(interactor.gravity.knobs.diversity).isWithin(1e-6).of(0.7)
        assertThat(interactor.gravity.knobs.mode).isEqualTo("explore")
        assertThat(viewModel.state.value.knobs.isDefault).isFalse()

        viewModel.resetKnobs()
        assertThat(interactor.gravity.knobs).isEqualTo(GravityKnobs())
    }

    @Test
    fun `explore is refused while a destination is set and dropped when one is picked`() = runTest {
        interactor.state.value = queue(Gravity(knobs = GravityKnobs(mode = "explore")))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.openSearch(SteeringSearchTarget.Destination)
        viewModel.pickSearchResult(SteeringReference("album", "al1", "Album"))
        advanceUntilIdle()
        assertThat(interactor.gravity.destination?.entityId).isEqualTo("al1")
        assertThat(interactor.gravity.knobs.mode).isNull()
        assertThat(viewModel.state.value.search).isNull()

        viewModel.setMode(SteeringKnobs.MODE_EXPLORE)
        assertThat(interactor.gravity.knobs.mode).isNull()
    }

    @Test
    fun `criterion weights materialise the effective defaults`() = runTest {
        interactor.options = RadioOptions(
            criteria = listOf(
                RadioCriterionOption("musicfm.mean.v1", "Sound profile"),
                RadioCriterionOption("ast.audioset.v1", "Audio scene"),
            ),
        )
        interactor.state.value = queue(Gravity())
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()
        assertThat(viewModel.state.value.knobs.criteria.map { it.weight }).containsExactly(1f, 0f).inOrder()

        viewModel.setCriterionWeight("ast.audioset.v1", 0.5f)
        assertThat(interactor.gravity.knobs.criteria).containsExactly(
            GravityCriterion("musicfm.mean.v1", 1.0),
            GravityCriterion("ast.audioset.v1", 0.5),
        )
    }

    @Test
    fun `search adds away references and source anchors`() = runTest {
        interactor.state.value = queue(Gravity())
        interactor.results = listOf(SteeringReference("artist", "a9", "Nine"))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.openSearch(SteeringSearchTarget.Away)
        viewModel.updateSearchQuery("nine")
        advanceUntilIdle()
        assertThat(viewModel.state.value.search?.results).isEqualTo(interactor.results)
        viewModel.pickSearchResult(interactor.results!!.single())
        assertThat(interactor.gravity.knobs.away.map { it.entityId }).containsExactly("a9")

        viewModel.openSearch(SteeringSearchTarget.Source)
        viewModel.pickSearchResult(SteeringReference("track", "t9", "Track"))
        assertThat(interactor.gravity.source.kind).isEqualTo(GravitySource.KIND_REFERENCES)
        assertThat(interactor.gravity.source.references.map { it.entityId }).containsExactly("t9")

        viewModel.resetSourceToQueue()
        assertThat(interactor.gravity.source).isEqualTo(GravitySource())
    }

    @Test
    fun `failed search reports an error`() = runTest {
        interactor.state.value = queue(Gravity())
        interactor.results = null
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.openSearch(SteeringSearchTarget.Destination)
        viewModel.updateSearchQuery("x")
        advanceUntilIdle()
        assertThat(viewModel.state.value.search?.isError).isTrue()
    }

    private class FakeInteractor : SteeringScreenViewModel.Interactor {
        val state = MutableStateFlow(PlaybackGravityState())
        val smartEnabled = MutableStateFlow(true)
        var options: RadioOptions? = null
        var results: List<SteeringReference>? = emptyList()

        val gravity: Gravity get() = state.value.gravity!!

        override fun getGravityState(): Flow<PlaybackGravityState> = state
        override fun getSmartContinuationEnabled(): Flow<Boolean> = smartEnabled
        override suspend fun setSmartContinuationEnabled(enabled: Boolean) {
            smartEnabled.value = enabled
        }

        override fun updateGravity(transform: (Gravity) -> Gravity) {
            val current = state.value.gravity ?: return
            state.value = state.value.copy(gravity = transform(current))
        }

        override suspend fun getRadioOptions(): RadioOptions? = options
        override suspend fun searchReferences(query: String): List<SteeringReference>? = results
    }
}
