package com.lelloman.pezzottify.android.domain.player.internal

import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.player.TrackMetadata
import com.lelloman.pezzottify.android.domain.statics.Album
import com.lelloman.pezzottify.android.domain.statics.Artist
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.Track
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.channelFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.launch
import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds

/**
 * Snapshot of a queue's metadata as resolved so far.
 *
 * @property tracks one entry per requested track id, in order; null while the track itself
 * is not loaded yet. A loaded track whose album/artists are still pending has empty album
 * name, artwork and artist names until they arrive.
 * @property isComplete true once every track, album and artist has been loaded.
 */
internal data class ResolvedQueueMetadata(
    val tracks: List<TrackMetadata?>,
    val isComplete: Boolean,
)

/**
 * Resolves the metadata of a queue of track ids from [StaticsProvider], reactively.
 *
 * On a cold cache the statics chain (DB miss -> synchronizer fetch -> DB write -> flow emits)
 * can be slow, so instead of a one-shot snapshot with a hard timeout, every item is observed
 * until it is loaded and the result is re-emitted as items arrive:
 *
 * - The first emission happens when everything is resolved, or after [firstPassTimeout],
 *   whichever comes first (so callers can keep showing a loading state briefly instead of
 *   flashing placeholders when the data is a few milliseconds away).
 * - After that, every batch of newly loaded items produces a new emission.
 * - Each statics flow is collected only until its first `Loaded` value, so long-lived
 *   observers (Room invalidation listeners) only exist for items that are still missing.
 * - The flow completes once everything is resolved; items that never load (or whose flow
 *   fails) keep it active
 *   until the collector is cancelled (e.g. the playlist changes).
 *
 * All state is mutated by the single consuming loop, so the flow is safe to collect on
 * multi-threaded dispatchers. Rebuilding a snapshot is O(tracks) and happens once per batch
 * of events, not once per event.
 */
internal class QueueMetadataResolver(
    private val staticsProvider: StaticsProvider,
    private val configStore: ConfigStore,
    private val firstPassTimeout: Duration = DEFAULT_FIRST_PASS_TIMEOUT,
) {

    private sealed interface Event {
        data class TrackLoaded(val id: String, val track: Track) : Event
        data class AlbumLoaded(val id: String, val album: Album) : Event
        data class ArtistLoaded(val id: String, val artist: Artist) : Event
        data object FirstPassTimeout : Event
    }

    fun observe(trackIds: List<String>): Flow<ResolvedQueueMetadata> = channelFlow {
        val events = Channel<Event>(Channel.UNLIMITED)
        val tracks = HashMap<String, Track>()
        val albums = HashMap<String, Album>()
        val artists = HashMap<String, Artist>()
        val requestedAlbums = HashSet<String>()
        val requestedArtists = HashSet<String>()
        var pending = 0

        fun <T> resolve(flow: Flow<StaticsItem<out T>>, toEvent: (T) -> Event) {
            pending++
            launch {
                // A failing or completing upstream leaves the item unresolved (placeholder)
                // instead of tearing down the whole queue.
                @Suppress("UNCHECKED_CAST")
                val loaded = flow
                    .catch { }
                    .firstOrNull { it is StaticsItem.Loaded } as StaticsItem.Loaded<out T>?
                if (loaded != null) events.send(toEvent(loaded.data))
            }
        }

        fun snapshot(): ResolvedQueueMetadata {
            val baseUrl = configStore.baseUrl.value.trimEnd('/')
            return ResolvedQueueMetadata(
                tracks = trackIds.map { id ->
                    tracks[id]?.let { track -> buildMetadata(id, track, albums, artists, baseUrl) }
                },
                isComplete = pending == 0,
            )
        }

        trackIds.toSet().forEach { id ->
            resolve(staticsProvider.provideTrack(id)) { Event.TrackLoaded(id, it) }
        }

        var firstPassDone = false
        val timeoutJob = launch {
            delay(firstPassTimeout)
            events.send(Event.FirstPassTimeout)
        }

        while (pending > 0) {
            var event: Event? = events.receive()
            while (event != null) {
                when (event) {
                    is Event.TrackLoaded -> {
                        pending--
                        tracks[event.id] = event.track
                        val albumId = event.track.albumId
                        if (albumId.isNotEmpty() && requestedAlbums.add(albumId)) {
                            resolve(staticsProvider.provideAlbum(albumId)) { Event.AlbumLoaded(albumId, it) }
                        }
                        event.track.artistsIds.forEach { artistId ->
                            if (requestedArtists.add(artistId)) {
                                resolve(staticsProvider.provideArtist(artistId)) {
                                    Event.ArtistLoaded(artistId, it)
                                }
                            }
                        }
                    }
                    is Event.AlbumLoaded -> {
                        pending--
                        albums[event.id] = event.album
                    }
                    is Event.ArtistLoaded -> {
                        pending--
                        artists[event.id] = event.artist
                    }
                    Event.FirstPassTimeout -> firstPassDone = true
                }
                event = events.tryReceive().getOrNull()
            }
            if (firstPassDone || pending == 0) {
                firstPassDone = true
                send(snapshot())
            }
        }
        timeoutJob.cancel()
        if (!firstPassDone) {
            // Nothing to resolve (empty queue).
            send(snapshot())
        }
    }

    private fun buildMetadata(
        trackId: String,
        track: Track,
        albums: Map<String, Album>,
        artists: Map<String, Artist>,
        baseUrl: String,
    ): TrackMetadata {
        val album = albums[track.albumId]
        return TrackMetadata(
            trackId = trackId,
            trackName = track.name,
            artistNames = track.artistsIds.mapNotNull { artists[it]?.name },
            primaryArtistId = track.artistsIds.firstOrNull() ?: "",
            albumId = track.albumId,
            albumName = album?.name ?: "",
            artworkUrl = album?.displayImageId?.let { "$baseUrl/v1/content/image/$it" },
            imageId = album?.displayImageId,
            durationSeconds = track.durationSeconds,
            availability = track.availability,
        )
    }

    companion object {
        val DEFAULT_FIRST_PASS_TIMEOUT: Duration = 2.seconds

        /** Placeholder for a track whose metadata has not been loaded yet. */
        fun placeholder(trackId: String) = TrackMetadata(
            trackId = trackId,
            trackName = trackId,
            artistNames = emptyList(),
            albumId = "",
            albumName = "",
            artworkUrl = null,
            durationSeconds = 0,
        )
    }
}
