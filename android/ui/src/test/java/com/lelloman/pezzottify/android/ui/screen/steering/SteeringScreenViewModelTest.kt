package com.lelloman.pezzottify.android.ui.screen.steering

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.player.Gravity
import com.lelloman.pezzottify.android.domain.player.GravityComponentDiagnostics
import com.lelloman.pezzottify.android.domain.player.GravityCriterion
import com.lelloman.pezzottify.android.domain.player.GravityDiagnostics
import com.lelloman.pezzottify.android.domain.player.GravityKnobs
import com.lelloman.pezzottify.android.domain.player.GravityNamespaceDiagnostics
import com.lelloman.pezzottify.android.domain.player.GravityReference
import com.lelloman.pezzottify.android.domain.player.GravitySource
import com.lelloman.pezzottify.android.domain.player.PlaybackGravityState
import com.lelloman.pezzottify.android.domain.remoteapi.response.Concept
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
        // The artwork collage previews only the tracks the user chose.
        assertThat(state.source).isEqualTo(
            SteeringSource.Queue(userChosenCount = 2, suggestedCount = 1, previewTrackIds = listOf("t1", "t2")),
        )
        assertThat(state.destination).isNull()
    }

    @Test
    fun `destination shows progress, remaining steps and similarity`() = runTest {
        val diagnostics = GravityDiagnostics(
            namespaces = listOf(GravityNamespaceDiagnostics("musicfm.mean.v1", queryToDestination = 0.42)),
        )
        interactor.state.value = queue(
            Gravity(destination = listOf(artist), stepsTotal = 10, stepsDone = 4, lastDiagnostics = diagnostics),
        )
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        val state = viewModel.state.value
        assertThat(state.destination?.components?.single()?.reference?.label).isEqualTo("Artist One")
        assertThat(state.destination?.progress).isWithin(1e-6f).of(0.4f)
        assertThat(state.destination?.queryToDestination).isWithin(1e-6f).of(0.42f)
        assertThat(state.stepsRemaining).isEqualTo(6)
    }

    @Test
    fun `editing remaining steps keeps steps done and can arrive`() = runTest {
        interactor.state.value = queue(Gravity(destination = listOf(artist), stepsTotal = 10, stepsDone = 4))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.setStepsRemaining(3)
        assertThat(interactor.gravity.stepsTotal).isEqualTo(7)
        assertThat(interactor.gravity.stepsDone).isEqualTo(4)
        assertThat(interactor.gravity.destination).containsExactly(artist)

        viewModel.setStepsRemaining(0)
        // Remaining is clamped to one step, so the destination is still ahead.
        assertThat(interactor.gravity.stepsTotal).isEqualTo(5)
    }

    @Test
    fun `clear destination keeps the source`() = runTest {
        interactor.state.value = queue(Gravity(destination = listOf(artist)))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.clearDestination()
        assertThat(interactor.gravity.destination).isNull()
        assertThat(interactor.gravity.source.kind).isEqualTo(GravitySource.KIND_QUEUE)
    }

    @Test
    fun `along the way settings write gravity knobs and reset clears them`() = runTest {
        interactor.state.value = queue(Gravity())
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()
        val defaults = viewModel.state.value.knobs
        assertThat(defaults.isDefault).isTrue()
        assertThat(defaults.recencyWeight).isEqualTo(SteeringKnobs.DEFAULT_RECENCY_WEIGHT)
        assertThat(defaults.variety).isWithin(1e-6f).of(SteeringKnobs.DEFAULT_VARIETY)

        viewModel.setRecencyWeight(0.5f)
        viewModel.setVariety(0.7f)
        advanceUntilIdle()
        assertThat(interactor.gravity.knobs.recencyWeight).isWithin(1e-6).of(0.5)
        // One "Variety" setting drives both fields.
        assertThat(interactor.gravity.knobs.diversity).isWithin(1e-6).of(0.7)
        assertThat(interactor.gravity.knobs.randomness).isWithin(1e-6).of(0.7)
        assertThat(viewModel.state.value.knobs.variety).isWithin(1e-6f).of(0.7f)
        assertThat(viewModel.state.value.knobs.isDefault).isFalse()

        viewModel.resetKnobs()
        advanceUntilIdle()
        assertThat(interactor.gravity.knobs).isEqualTo(GravityKnobs())
        assertThat(viewModel.state.value.knobs.isDefault).isTrue()
    }

    @Test
    fun `variety shows the average of older separate settings`() = runTest {
        interactor.state.value = queue(Gravity(knobs = GravityKnobs(diversity = 0.2, randomness = 0.6)))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()
        assertThat(viewModel.state.value.knobs.variety).isWithin(1e-6f).of(0.4f)
    }

    @Test
    fun `explore mode left over from earlier is cleared by any edit and by picking a destination`() = runTest {
        interactor.state.value = queue(Gravity(knobs = GravityKnobs(mode = "explore")))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.setRecencyWeight(0.3f)
        assertThat(interactor.gravity.knobs.mode).isNull()

        interactor.state.value = queue(interactor.gravity.withKnobs(GravityKnobs(mode = "explore")))
        advanceUntilIdle()
        // Only a leftover mode: the settings still read as defaults.
        assertThat(viewModel.state.value.knobs.isDefault).isTrue()
        viewModel.openSearch(SteeringSearchTarget.Destination)
        viewModel.pickSearchResult(SteeringReference("album", "al1", "Album"))
        advanceUntilIdle()
        assertThat(interactor.gravity.destination?.single()?.entityId).isEqualTo("al1")
        assertThat(interactor.gravity.knobs.mode).isNull()
        assertThat(viewModel.state.value.search).isNull()
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
        assertThat(viewModel.state.value.knobs.criteria.map { it.kind }).containsExactly(
            SteeringCriterionKind.OverallSound,
            SteeringCriterionKind.AudioScene,
        ).inOrder()

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

    @Test
    fun `mix components show weights and per component closeness and can be edited`() = runTest {
        val jazz = GravityReference("concept", "audioset:Jazz", "Jazz", weight = 0.5)
        val diagnostics = GravityDiagnostics(
            namespaces = listOf(
                GravityNamespaceDiagnostics(
                    "musicfm.mean.v1",
                    destinationComponents = listOf(
                        GravityComponentDiagnostics("artist", "a1", 0.3),
                        GravityComponentDiagnostics("concept", "audioset:Jazz", null),
                    ),
                ),
            ),
        )
        interactor.state.value = queue(
            Gravity(destination = listOf(artist, jazz), stepsTotal = 10, stepsDone = 2, lastDiagnostics = diagnostics),
        )
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        val components = viewModel.state.value.destination!!.components
        assertThat(components.map { it.reference.label }).containsExactly("Artist One", "Jazz").inOrder()
        assertThat(components[0].similarity).isWithin(1e-6f).of(0.3f)
        assertThat(components[1].similarity).isNull()
        assertThat(components[1].weight).isWithin(1e-6f).of(0.5f)

        viewModel.setDestinationComponentWeight(components[1].reference, 1.5f)
        assertThat(interactor.gravity.destination!!.last().weight).isWithin(1e-6).of(1.5)
        assertThat(interactor.gravity.stepsDone).isEqualTo(2)

        viewModel.removeDestinationComponent(components[0].reference)
        assertThat(interactor.gravity.destination!!.map { it.entityId }).containsExactly("audioset:Jazz")
        viewModel.removeDestinationComponent(components[1].reference)
        assertThat(interactor.gravity.destination).isNull()
    }

    @Test
    fun `search offers concepts grouped by family and picking one grows the destination mix`() = runTest {
        interactor.concepts = listOf(
            Concept("recorded:1960s", Concept.FAMILY_RECORDED, "Recorded in the 1960s", 200),
            Concept("audioset:Piano", Concept.FAMILY_INSTRUMENT, "Piano", 200),
            Concept("audioset:Jazz", Concept.FAMILY_SOUND_GENRE, "Jazz", 200),
            Concept("genre:jazz fusion", Concept.FAMILY_GENRE_TAG, "jazz fusion", 80),
        )
        interactor.state.value = queue(Gravity(destination = listOf(artist)))
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.openSearch(SteeringSearchTarget.Destination)
        advanceUntilIdle()
        val browse = viewModel.state.value.search!!
        assertThat(browse.isLoadingConcepts).isFalse()
        assertThat(browse.concepts.map { it.family }).containsExactly(
            Concept.FAMILY_SOUND_GENRE, Concept.FAMILY_INSTRUMENT, Concept.FAMILY_GENRE_TAG, Concept.FAMILY_RECORDED,
        ).inOrder()

        viewModel.updateSearchQuery("jazz")
        advanceUntilIdle()
        val filtered = viewModel.state.value.search!!.concepts
        assertThat(filtered.flatMap { group -> group.concepts.map { it.entityId } })
            .containsExactly("audioset:Jazz", "genre:jazz fusion").inOrder()

        viewModel.pickSearchResult(filtered.first().concepts.single())
        assertThat(interactor.gravity.destination!!.map { it.entityId }).containsExactly("a1", "audioset:Jazz").inOrder()
        assertThat(interactor.gravity.destination!!.last().entityType).isEqualTo("concept")

        // Concepts are loaded once and reused for later searches.
        viewModel.openSearch(SteeringSearchTarget.Away)
        advanceUntilIdle()
        assertThat(interactor.conceptLoads).isEqualTo(1)
    }

    @Test
    fun `concepts that fail to load leave item search working`() = runTest {
        interactor.concepts = null
        interactor.state.value = queue(Gravity())
        val viewModel = SteeringScreenViewModel(interactor)
        advanceUntilIdle()

        viewModel.openSearch(SteeringSearchTarget.Destination)
        advanceUntilIdle()
        val search = viewModel.state.value.search!!
        assertThat(search.isLoadingConcepts).isFalse()
        assertThat(search.concepts).isEmpty()
    }

    private class FakeInteractor : SteeringScreenViewModel.Interactor {
        val state = MutableStateFlow(PlaybackGravityState())
        val smartEnabled = MutableStateFlow(true)
        var options: RadioOptions? = null
        var results: List<SteeringReference>? = emptyList()
        var concepts: List<Concept>? = emptyList()
        var conceptLoads = 0

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
        override suspend fun getConcepts(): List<Concept>? {
            conceptLoads++
            return concepts
        }
    }

    @Test
    fun `criteria are named by what they compare`() {
        assertThat(criterionKind("musicfm.mean.v1")).isEqualTo(SteeringCriterionKind.OverallSound)
        assertThat(criterionKind("ast.audioset.v2")).isEqualTo(SteeringCriterionKind.AudioScene)
        assertThat(criterionKind("ast.instruments.v2")).isEqualTo(SteeringCriterionKind.Instruments)
        assertThat(criterionKind("custom.ns")).isEqualTo(SteeringCriterionKind.Other)
    }
}
