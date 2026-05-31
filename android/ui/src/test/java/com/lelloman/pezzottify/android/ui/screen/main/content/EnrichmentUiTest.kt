package com.lelloman.pezzottify.android.ui.screen.main.content

import com.google.common.truth.Truth.assertThat
import java.util.Locale
import org.junit.After
import org.junit.Before
import org.junit.Test

class EnrichmentUiTest {

    private lateinit var defaultLocale: Locale

    @Before
    fun setUp() {
        defaultLocale = Locale.getDefault()
        Locale.setDefault(Locale.US)
    }

    @After
    fun tearDown() {
        Locale.setDefault(defaultLocale)
    }

    @Test
    fun `formatEnrichmentDate keeps years unchanged`() {
        assertThat(formatEnrichmentDate("1999")).isEqualTo("1999")
    }

    @Test
    fun `formatEnrichmentDate formats year and month`() {
        assertThat(formatEnrichmentDate("1999-02")).isEqualTo("February 1999")
    }

    @Test
    fun `formatEnrichmentDate formats full dates`() {
        assertThat(formatEnrichmentDate("1999-02-03")).isEqualTo("Feb 3, 1999")
    }

    @Test
    fun `formatEnrichmentDate keeps invalid dates unchanged`() {
        assertThat(formatEnrichmentDate("1999-13")).isEqualTo("1999-13")
        assertThat(formatEnrichmentDate("1999-02-31")).isEqualTo("1999-02-31")
    }
}
