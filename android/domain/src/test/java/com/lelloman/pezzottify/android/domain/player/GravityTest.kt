package com.lelloman.pezzottify.android.domain.player

import com.google.common.truth.Truth.assertThat
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import org.junit.Test

class GravityTest {
    private fun ids(count: Int, prefix: String = "t") = (0 until count).map { "$prefix$it" }
    private val artist = GravityReference("artist", "a1", label = "A")

    @Test fun `defaults match the shared schema`() {
        val gravity = Gravity()
        assertThat(gravity.v).isEqualTo(1)
        assertThat(gravity.source).isEqualTo(GravitySource(kind = "queue"))
        assertThat(gravity.autoTrackIds).isEmpty()
        assertThat(gravity.destination).isNull()
        assertThat(gravity.stepsTotal).isEqualTo(20)
        assertThat(gravity.stepsDone).isEqualTo(0)
        assertThat(gravity.knobs).isEqualTo(GravityKnobs())
        assertThat(gravity.knobs.recencyWeight).isNull()
        assertThat(gravity.lastDiagnostics).isNull()
        assertThat(gravity.progress()).isEqualTo(0.0)
    }

    @Test fun `appendedAuto records provenance deduped and capped`() {
        val gravity = Gravity().appendedAuto(listOf("a", "b")).appendedAuto(listOf("b", "c"))
        assertThat(gravity.autoTrackIds).containsExactly("a", "b", "c").inOrder()
        assertThat(gravity.isAuto("b")).isTrue()
        assertThat(gravity.isAuto("z")).isFalse()
        val capped = Gravity().appendedAuto(ids(1200))
        assertThat(capped.autoTrackIds).hasSize(Gravity.AUTO_IDS_CAP)
        assertThat(capped.autoTrackIds.last()).isEqualTo("t1199")
    }

    @Test fun `userAdded promotes a suggestion and removed keeps exclusion`() {
        val gravity = Gravity().appendedAuto(listOf("a", "b"))
        assertThat(gravity.userAdded(listOf("a")).autoTrackIds).containsExactly("b")
        assertThat(gravity.removed("a")).isEqualTo(gravity)
    }

    @Test fun `destination counts auto appends and arrives at steps total`() {
        var gravity = Gravity().withDestination(artist, stepsTotal = 3)
        assertThat(gravity.stepsDone).isEqualTo(0)
        assertThat(gravity.progress()).isEqualTo(0.0)
        gravity = gravity.appendedAuto(listOf("x", "y"))
        assertThat(gravity.stepsDone).isEqualTo(2)
        assertThat(gravity.progress()).isWithin(1e-9).of(2.0 / 3.0)
        assertThat(gravity.destination).isEqualTo(artist)
        gravity = gravity.appendedAuto(listOf("z", "w"))
        assertThat(gravity.destination).isNull()
        assertThat(gravity.stepsDone).isEqualTo(0)
        assertThat(gravity.stepsTotal).isEqualTo(3)
        assertThat(gravity.source.kind).isEqualTo("references")
        assertThat(gravity.source.references).containsExactly(artist.copy(weight = 1.0))
        assertThat(gravity.autoTrackIds).containsExactly("x", "y", "z", "w").inOrder()
    }

    @Test fun `setting a destination resets steps and clearing keeps source`() {
        val arrived = Gravity().withDestination(artist, 1).appendedAuto(listOf("x"))
        val next = arrived.withDestination(GravityReference("album", "al1"), 5)
        assertThat(next.stepsDone).isEqualTo(0)
        assertThat(next.stepsTotal).isEqualTo(5)
        assertThat(next.source.kind).isEqualTo("references")
        val cleared = next.withDestination(null)
        assertThat(cleared.destination).isNull()
        assertThat(cleared.source).isEqualTo(next.source)
        assertThat(cleared.stepsTotal).isEqualTo(5)
        assertThat(Gravity().withDestination(artist, 0).stepsTotal).isEqualTo(1)
    }

    @Test fun `lowering steps total below steps done arrives`() {
        val gravity = Gravity().withDestination(artist, 10).appendedAuto(listOf("x", "y", "z"))
        val same = gravity.withStepsTotal(5)
        assertThat(same.destination).isEqualTo(artist)
        assertThat(same.stepsTotal).isEqualTo(5)
        val arrived = gravity.withStepsTotal(3)
        assertThat(arrived.destination).isNull()
        assertThat(arrived.source.references).containsExactly(artist.copy(weight = 1.0))
    }

    @Test fun `knobs source and diagnostics setters`() {
        val knobs = GravityKnobs(recencyWeight = 0.5, mode = "explore", away = listOf(artist))
        val diagnostics = GravityDiagnostics(recencyWeight = 0.2, progress = null, at = 7)
        val gravity = Gravity().withKnobs(knobs).withSource(GravitySource("references", listOf(artist))).withDiagnostics(diagnostics)
        assertThat(gravity.knobs).isEqualTo(knobs)
        assertThat(gravity.source.references).containsExactly(artist)
        assertThat(gravity.lastDiagnostics).isEqualTo(diagnostics)
    }

    @Test fun `sampleEvenly keeps order and first element`() {
        val items = ids(1000)
        assertThat(Gravity.sampleEvenly(ids(5), 200)).isEqualTo(ids(5))
        val sampled = Gravity.sampleEvenly(items, 200)
        assertThat(sampled).hasSize(200)
        assertThat(sampled.first()).isEqualTo("t0")
        assertThat(sampled.map { items.indexOf(it) }).isInStrictOrder()
    }

    @Test fun `request anchors on user chosen tracks and excludes suggestions`() {
        val queue = ids(8) + listOf("s1", "s2")
        val gravity = Gravity().appendedAuto(listOf("s1", "s2", "removed"))
        val request = gravity.buildContinuationRequest(queue, currentIndex = 8, count = 2)
        assertThat(request.contextTrackIds).isEqualTo(queue)
        assertThat(request.sourceTrackIds).isEqualTo(ids(8))
        assertThat(request.sourceReferences).isEmpty()
        assertThat(request.recentTrackIds).containsExactly("t4", "t5", "t6", "t7", "s1").inOrder()
        assertThat(request.excludeTrackIds).containsExactlyElementsIn(queue + "removed")
        assertThat(request.count).isEqualTo(2)
        assertThat(request.recencyWeight).isNull()
        assertThat(request.destination).isNull()
        assertThat(request.progress).isNull()
        assertThat(request.criteria).isNull()
        assertThat(request.diversity).isNull()
        assertThat(request.randomness).isNull()
        assertThat(request.mode).isNull()
        assertThat(request.away).isEmpty()
    }

    @Test fun `request samples long sources and clamps recent window at the start`() {
        val queue = ids(600)
        val request = Gravity().buildContinuationRequest(queue, currentIndex = 1, count = 1)
        assertThat(request.sourceTrackIds).hasSize(Gravity.SOURCE_SAMPLE_CAP)
        assertThat(request.recentTrackIds).containsExactly("t0", "t1").inOrder()
        assertThat(request.contextTrackIds).isEqualTo(queue.takeLast(10))
        assertThat(Gravity().buildContinuationRequest(emptyList(), 0, 1).recentTrackIds).isEmpty()
    }

    @Test fun `request carries destination progress knobs and reference sources`() {
        val gravity = Gravity()
            .withDestination(artist, 4)
            .appendedAuto(listOf("s1"))
            .withKnobs(GravityKnobs(recencyWeight = 0.1, criteria = listOf(GravityCriterion("ns", 0.7)), diversity = 0.2, randomness = 0.0, mode = "similar", away = listOf(GravityReference("track", "x", weight = 2.0))))
        val request = gravity.buildContinuationRequest(listOf("a", "s1"), 1, 3)
        assertThat(request.destination?.entityType).isEqualTo("artist")
        assertThat(request.destination?.entityId).isEqualTo("a1")
        assertThat(request.progress).isWithin(1e-9).of(0.25)
        assertThat(request.recencyWeight).isEqualTo(0.1)
        assertThat(request.criteria?.single()?.namespace).isEqualTo("ns")
        assertThat(request.diversity).isEqualTo(0.2)
        assertThat(request.randomness).isEqualTo(0.0)
        assertThat(request.mode).isEqualTo("similar")
        assertThat(request.away.single().weight).isEqualTo(2.0)

        val arrived = gravity.withStepsTotal(1)
        val afterArrival = arrived.buildContinuationRequest(listOf("a", "s1"), 1, 1)
        assertThat(afterArrival.sourceTrackIds).isEmpty()
        assertThat(afterArrival.sourceReferences.single().entityId).isEqualTo("a1")
        assertThat(afterArrival.destination).isNull()
    }

    @Test fun `wire json round trips and includes defaults for the web client`() {
        val gravity = Gravity().withDestination(artist, 7).appendedAuto(listOf("t1", "t2"))
            .withKnobs(GravityKnobs(recencyWeight = 0.3))
        val wire = gravity.toWireJson()
        assertThat(wire["steps_total"].toString()).isEqualTo("7")
        assertThat(wire["steps_done"].toString()).isEqualTo("2")
        assertThat(wire["auto_track_ids"].toString()).isEqualTo("[\"t1\",\"t2\"]")
        assertThat(wire["source"]!!.jsonObject["kind"].toString()).isEqualTo("\"queue\"")
        assertThat(wire["destination"]!!.jsonObject["entity_type"].toString()).isEqualTo("\"artist\"")
        assertThat(wire["knobs"]!!.jsonObject["recency_weight"].toString()).isEqualTo("0.3")
        assertThat(wire["knobs"]!!.jsonObject["diversity"].toString()).isEqualTo("null")
        assertThat(wire["last_diagnostics"].toString()).isEqualTo("null")
        assertThat(Gravity.fromWireJson(wire)).isEqualTo(gravity)
    }

    @Test fun `web produced fixture decodes with defaults for missing keys`() {
        val fixture = """{"v":1,"source":{"kind":"queue"},"auto_track_ids":["t1"],"destination":{"entity_type":"artist","entity_id":"a1","label":"A"},"steps_total":7,"steps_done":2,"knobs":{"recency_weight":null,"criteria":null,"diversity":null,"randomness":null,"mode":null,"away":[]},"last_diagnostics":null}"""
        val gravity = Gravity.fromWireJson(Json.parseToJsonElement(fixture).jsonObject)
        assertThat(gravity.autoTrackIds).containsExactly("t1")
        assertThat(gravity.destination).isEqualTo(GravityReference("artist", "a1", label = "A", weight = 1.0))
        assertThat(gravity.stepsTotal).isEqualTo(7)
        assertThat(gravity.stepsDone).isEqualTo(2)
        assertThat(gravity.source.references).isEmpty()
        assertThat(gravity.knobs).isEqualTo(GravityKnobs())
        val minimal = Gravity.fromWireJson(Json.parseToJsonElement("""{"unknown":true}""").jsonObject)
        assertThat(minimal).isEqualTo(Gravity())
    }
}
