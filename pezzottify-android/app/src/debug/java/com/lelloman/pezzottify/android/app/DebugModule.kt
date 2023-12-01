package com.lelloman.pezzottify.android.app

import com.lelloman.debuginterface.DebugServerBuilder
import com.lelloman.pezzottify.android.app.debugcommands.makeDebugOperationsList
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.domain.statics.StaticsStore
import com.lelloman.pezzottify.android.app.initializer.AppInitializer
import com.lelloman.pezzottify.android.app.initializer.DebugServerInitializer
import com.lelloman.pezzottify.android.app.ui.Navigator
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class DebugModule {

    @Provides
    @Singleton
    fun provideDebugServerBuilder(
        loginManager: LoginManager,
        staticsStore: StaticsStore,
        navigator: Navigator,
    ) = DebugServerBuilder().apply {
        val operations = makeDebugOperationsList(
            loginManager = loginManager,
            staticsStore = staticsStore,
            navigator = navigator,
        )
        operations.forEach { op -> add(op) }
    }

    @Provides
    @IntoSet
    fun providesDebugServerAppInitializer(
        debugServerBuilder: DebugServerBuilder
    ): AppInitializer = DebugServerInitializer(debugServerBuilder)
}