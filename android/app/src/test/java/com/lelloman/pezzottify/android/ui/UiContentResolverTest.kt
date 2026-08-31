package com.lelloman.pezzottify.android.ui

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.config.ConfigStore
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.statics.StaticsItem
import com.lelloman.pezzottify.android.domain.statics.StaticsProvider
import com.lelloman.pezzottify.android.domain.statics.Track
import com.lelloman.pezzottify.android.domain.statics.TrackAvailability as DomainTrackAvailability
import com.lelloman.pezzottify.android.ui.content.Content
import com.lelloman.pezzottify.android.ui.content.TrackAvailability
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Test

class UiContentResolverTest {

    @Test
    fun `resolved missing track reacts when proxy becomes available`() = runTest {
        val proxyAvailable = MutableStateFlow(false)
        val proxyEnabled = MutableStateFlow(true)
        val settings = mockk<UserSettingsStore> {
            every { isProxyStreamingAvailable } returns proxyAvailable
            every { isProxyModeEnabled } returns proxyEnabled
        }
        val track = mockk<Track> {
            every { id } returns "track-1"
            every { name } returns "Missing track"
            every { albumId } returns "album-1"
            every { artistsIds } returns emptyList()
            every { durationSeconds } returns 180
            every { availability } returns DomainTrackAvailability.Unavailable
            every { enrichmentStatus } returns null
            every { enrichment } returns null
        }
        val staticsProvider = mockk<StaticsProvider> {
            every { provideTrack("track-1") } returns
                flowOf(StaticsItem.Loaded("track-1", track))
        }
        val configStore = mockk<ConfigStore> {
            every { baseUrl } returns MutableStateFlow("https://example.test")
        }
        val resolver = UiContentResolver(staticsProvider, configStore, settings)

        val emissions = async {
            resolver.resolveTrack("track-1").take(2).toList()
        }
        runCurrent()
        proxyAvailable.value = true

        val availabilities = emissions.await().map {
            (it as Content.Resolved).data.availability
        }
        assertThat(availabilities).containsExactly(
            TrackAvailability.Unavailable,
            TrackAvailability.Available,
        ).inOrder()
    }
}
