package com.lelloman.pezzottify.android.domain.statics

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class TrackAvailabilityTest {

    @Test
    fun `fromServerString parses available`() {
        assertThat(TrackAvailability.fromServerString("available"))
            .isEqualTo(TrackAvailability.Available)
    }

    @Test
    fun `fromServerString parses unavailable`() {
        assertThat(TrackAvailability.fromServerString("unavailable"))
            .isEqualTo(TrackAvailability.Unavailable)
    }

    @Test
    fun `fromServerString parses fetching`() {
        assertThat(TrackAvailability.fromServerString("fetching"))
            .isEqualTo(TrackAvailability.Fetching)
    }

    @Test
    fun `fromServerString parses fetch_error`() {
        assertThat(TrackAvailability.fromServerString("fetch_error"))
            .isEqualTo(TrackAvailability.FetchError)
    }

    @Test
    fun `fromServerString returns Available for null`() {
        assertThat(TrackAvailability.fromServerString(null))
            .isEqualTo(TrackAvailability.Available)
    }

    @Test
    fun `fromServerString returns Available for unknown value`() {
        assertThat(TrackAvailability.fromServerString("unknown_state"))
            .isEqualTo(TrackAvailability.Available)
    }

    @Test
    fun `isPlayable returns true only for Available`() {
        assertThat(TrackAvailability.Available.isPlayable).isTrue()
        assertThat(TrackAvailability.Unavailable.isPlayable).isFalse()
        assertThat(TrackAvailability.Fetching.isPlayable).isFalse()
        assertThat(TrackAvailability.FetchError.isPlayable).isFalse()
    }
}
