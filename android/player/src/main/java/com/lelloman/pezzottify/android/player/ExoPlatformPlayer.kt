package com.lelloman.pezzottify.android.player

import android.content.Context
import android.os.Looper
import androidx.media3.common.MediaItem
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.okhttp.OkHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import okhttp3.OkHttpClient

@UnstableApi
internal class ExoPlatformPlayer(
    context: Context,
    private val authStore: AuthStore,
    looper: Looper,
) : PlatformPlayer {

    private val authToken get() = (authStore.getAuthState().value as? AuthState.LoggedIn)?.authToken.orEmpty()
    private val okHttpClient = OkHttpClient.Builder()
        .addInterceptor {
            it.proceed(
                it.request().newBuilder()
                    .addHeader("Authorization", authToken)
                    .build()
            )
        }
        .build()

    private val player = ExoPlayer
        .Builder(context).setMediaSourceFactory(
            DefaultMediaSourceFactory(context).setDataSourceFactory(
                DefaultDataSource.Factory(
                    context,
                    OkHttpDataSource.Factory {
                        okHttpClient.newCall(it)
                    }
                )
            )
        )
        .setLooper(looper)
        .build()

    override fun setIsPlaying(isPlaying: Boolean) {
        player.playWhenReady = isPlaying
    }

    override fun loadPlaylist(tracksUrls: List<String>) {
        player.clearMediaItems()
        tracksUrls.forEach {
            player.addMediaItem(MediaItem.fromUri(it))
        }
        player.prepare()
    }

    override fun loadTrackIndex(loadTrackIndex: Int) {
        player.seekTo(loadTrackIndex, 0)
    }

    override fun seekTrackProgressPercent(trackProgressPercent: Float) {
        TODO()
    }
}