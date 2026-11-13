package com.lelloman.pezzottify.android.updates

import android.app.Application
import android.content.Context
import com.lelloman.paravoidandroid.runtime.ParavoidUpdates
import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.domain.player.PlatformPlayer
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Tells the Paravoid shell the app is busy while music is playing on this device, so it holds
 * automatic restarts and restart prompts, and LelloStore (see `busyReportStores` in the build)
 * holds shell APK installs. A held restart goes ahead once playback stops, or after a day.
 *
 * Same rule as [UpdateConsentReceiver] in the regular APK: only local playback counts. Paused
 * playback is restored after the restart, and controlling another device keeps playing there.
 */
@Singleton
class ParavoidBusyReporter @Inject constructor(
    @ApplicationContext private val context: Context,
    private val platformPlayer: PlatformPlayer,
    private val scope: CoroutineScope,
) : AppInitializer {

    override fun initialize() {
        // The shell keeps the busy state in the main app process only.
        if (Application.getProcessName() != context.packageName) return
        scope.launch {
            platformPlayer.isPlaying.collect { ParavoidUpdates.get().setBusy(it) }
        }
    }
}

@Module
@InstallIn(SingletonComponent::class)
abstract class ParavoidBusyReporterModule {

    @Binds
    @IntoSet
    internal abstract fun bindBusyReporter(reporter: ParavoidBusyReporter): AppInitializer
}
