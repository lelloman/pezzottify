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
    val status get() = creation.status

    operator fun invoke(workId: String, trackId: String, title: String) {
        creation.start {
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
                val context = PlaybackPlaylistContext.Radio(
                    source = "work_versions", seedEntityType = "track", seedEntityId = trackId,
                    seedLabel = title, count = tracks.size,
                    settings = buildJsonObject { put("work_id", workId) },
                )
                val continuation = RadioContinuation.create(tracks, tracks)
                val commit: () -> Unit = { player.loadRadio(tracks, context, continuation) }
                commit
            }
        }
    }
}
