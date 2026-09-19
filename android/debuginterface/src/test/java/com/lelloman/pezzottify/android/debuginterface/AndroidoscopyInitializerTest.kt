package com.lelloman.pezzottify.android.debuginterface

import android.app.Application
import android.content.pm.ApplicationInfo
import com.lelloman.androidoscopy.Androidoscopy
import com.lelloman.androidoscopy.AndroidoscopyConfig
import com.lelloman.pezzottify.android.domain.auth.TokenRefresher
import com.lelloman.pezzottify.android.domain.cache.StaticsCache
import io.mockk.*
import org.junit.After
import org.junit.Assert.*
import org.junit.Test
import kotlin.time.Duration.Companion.minutes

class AndroidoscopyInitializerTest {
    @After fun tearDown() = unmockkAll()

    @Test fun `release initializes only a dormant session without debug collectors`() {
        val app = mockk<Application>()
        val info = mockk<ApplicationInfo>().apply { flags = 0 }
        every { app.applicationContext } returns app
        every { app.applicationInfo } returns info
        val cache = mockk<StaticsCache>()
        val refresher = mockk<TokenRefresher>()
        val diagnosticTools = mockk<PezzottifyDiagnosticTools>(relaxed = true)
        val configure = slot<AndroidoscopyConfig.() -> Unit>()
        mockkObject(Androidoscopy)
        every { Androidoscopy.init(app, capture(configure)) } just Runs

        AndroidoscopyInitializer(app, cache, refresher, diagnosticTools).initialize()

        verify(exactly = 1) { Androidoscopy.init(app, any()) }
        verify(exactly = 0) { Androidoscopy.startSession(any()) }
        verify(exactly = 0) { Androidoscopy.registerDataProvider(any()) }
        verify(exactly = 0) { Androidoscopy.registerTool(any()) }
        verify(exactly = 1) { diagnosticTools.register() }
        val config = AndroidoscopyConfig().apply(configure.captured)
        assertEquals("Pezzottify", config.appName)
        assertFalse(config.enableLogging)
        assertEquals(15.minutes, config.releaseIdleTimeout)
        verify { listOf(cache, refresher) wasNot Called }
    }
}
