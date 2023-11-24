package com.lelloman.pezzottify.android.app.localdata

import com.lelloman.pezzottify.android.app.domain.LoginOperation
import com.lelloman.pezzottify.android.app.domain.LoginState
import com.lelloman.pezzottify.android.localdata.StaticsDao
import com.lelloman.pezzottify.android.localdata.model.Album
import com.lelloman.pezzottify.android.localdata.model.AudioTrack
import com.lelloman.pezzottify.android.localdata.model.BandArtist
import com.lelloman.pezzottify.android.localdata.model.Image
import com.lelloman.pezzottify.android.localdata.model.IndividualArtist
import com.lelloman.pezzottify.remoteapi.RemoteApi
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext

private typealias RemoteIndividual = com.lelloman.pezzottify.remoteapi.model.IndividualArtist
private typealias RemoteBand = com.lelloman.pezzottify.remoteapi.model.BandArtist
private typealias RemoteImage = com.lelloman.pezzottify.remoteapi.model.Image
private typealias RemoteAudioTrack = com.lelloman.pezzottify.remoteapi.model.AudioTrack

class FetchStaticsLoginOperation(
    private val dispatcher: CoroutineDispatcher,
    private val remoteApi: RemoteApi,
    private val staticsDao: StaticsDao,
) : LoginOperation {

    private val RemoteAudioTrack.mapped
        get() = AudioTrack(
            id = id,
            size = size,
            name = name,
            durationMs = durationMs,
            sampleRate = sampleRate,
            bitRate = bitRate,
            type = when (type) {
                com.lelloman.pezzottify.remoteapi.model.AudioTrack.Type.MP3 -> AudioTrack.Type.MP3
                com.lelloman.pezzottify.remoteapi.model.AudioTrack.Type.FLAC -> AudioTrack.Type.FLAC
            }
        )
    private val RemoteBand.mapped
        get() = BandArtist(
            id = id,
            displayName = displayName,
            imageId = image?.id,
            membersIds = members.map { it.id },
        )

    private val RemoteIndividual.mapped
        get() = IndividualArtist(
            id = id,
            displayName = displayName,
            imageId = image?.id,
            firstName = firstName,
            lastName = lastName,
        )

    private val RemoteImage.mapped
        get() = Image(
            id = id,
            size = size,
            width = width,
            height = height,
            type = when (type) {
                com.lelloman.pezzottify.remoteapi.model.Image.Type.PNG -> Image.Type.PNG
                com.lelloman.pezzottify.remoteapi.model.Image.Type.JPG -> Image.Type.JPG
            }
        )

    override suspend fun invoke(loginState: LoginState.LoggedIn) = withContext(dispatcher) context@{
        val remoteAlbums = remoteApi.getAlbums().get() ?: return@context false
        val artists = remoteApi.getArtists().get() ?: return@context false
        val individuals = ArrayList<IndividualArtist>()
        val bands = ArrayList<BandArtist>()
        val images = ArrayList<Image>()
        val albums = ArrayList<Album>()
        val audioTracks = ArrayList<AudioTrack>()
        remoteAlbums.forEach { remoteAlbum ->
            val audioTracksIds = ArrayList<String>()
            remoteAlbum.audioTracks.forEach { remoteTrack ->
                audioTracksIds.add(remoteTrack.id)
                audioTracks.add(remoteTrack.mapped)
            }
            val localAlbum = Album(
                id = remoteAlbum.id,
                name = remoteAlbum.name,
                audioTracksIds = audioTracksIds,
                coverImageId = remoteAlbum.coverImage?.id,
                sideImagesIds = remoteAlbum.sideImages.map { it.id },
                artistsIds = remoteAlbum.artists.map { it.id },
            )
            albums.add(localAlbum)
        }
        artists.forEach { remoteArtist ->
            if (remoteArtist is RemoteIndividual) {
                individuals.add(remoteArtist.mapped)
            } else if (remoteArtist is RemoteBand) {
                bands.add(remoteArtist.mapped)
            }
            remoteArtist.image?.mapped?.let(images::add)
        }
        try {
            staticsDao.replaceStatics(
                albums = albums,
                individuals = individuals,
                bands = bands,
                images = images,
                audioTracks = audioTracks,
            )
            true
        } catch (e: Throwable) {
            e.printStackTrace()
            false
        }
    }

    private fun <T> RemoteApi.Response<T>.get(): T? = when (this) {
        is RemoteApi.Response.Success -> value
        else -> null
    }
}