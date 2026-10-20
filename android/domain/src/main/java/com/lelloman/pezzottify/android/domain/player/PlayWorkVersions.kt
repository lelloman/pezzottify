package com.lelloman.pezzottify.android.domain.player

import com.lelloman.pezzottify.android.domain.statics.TrackAvailability
import com.lelloman.pezzottify.android.domain.statics.usecase.GetWork
import javax.inject.Inject
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

class PlayWorkVersions @Inject constructor(
    private val getWork: GetWork,
    private val creation: RadioCreationController,
    private val player: PezzottifyPlayer,
) {
    val pendingPlayback get() = creation.pending

    operator fun invoke(workId: String, trackId: String, title: String) {
        val context = PlaybackPlaylistContext.Radio(
            source = "work_versions", seedEntityType = "track", seedEntityId = trackId,
            seedLabel = title, count = 0,
            settings = buildJsonObject { put("work_id", workId) },
        )
        creation.start(context) {
            val ids = linkedSetOf<String>()
            var offset = 0
            do {
                currentCoroutineContext().ensureActive()
                val page = getWork(workId, offset = offset, limit = 100)
                page.tracks.filter { it.track.availability == TrackAvailability.Available }
                    .forEach { ids.add(it.track.id) }
                offset = page.nextOffset
            } while (page.hasMore)
            if (ids.isEmpty()) null else {
                val tracks = if (trackId in ids) listOf(trackId) + ids.filter { it != trackId } else ids.toList()
                val continuation = RadioContinuation.create(tracks, tracks)
                val commit: RadioCommit = { finalContext ->
                    player.loadRadio(tracks, finalContext.copy(count = tracks.size), continuation)
                }
                commit
            }
        }
    }
}
