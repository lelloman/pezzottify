package com.lelloman.pezzottify.android.app.player

import android.content.Context
import android.os.Handler
import android.os.HandlerThread
import com.lelloman.pezzottify.android.app.domain.LoginManager
import com.lelloman.pezzottify.android.app.domain.LoginState
import com.lelloman.pezzottify.android.app.domain.LogoutOperation
import com.lelloman.pezzottify.android.log.LoggerFactory
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.android.asCoroutineDispatcher
import kotlinx.coroutines.flow.filterIsInstance
import kotlinx.coroutines.flow.map
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class PlayerModule {

    @Provides
    @Singleton
    fun providePlayerManager(
        @ApplicationContext context: Context,
        loginManager: LoginManager,
        loggerFactory: LoggerFactory,
    ): PlayerManager {
        val dispatcher = HandlerThread("Player thread")
            .apply { start() }
            .looper
            .let { Handler(it) }
            .asCoroutineDispatcher()
        return PlayerManagerImpl(
            loggerFactory = loggerFactory,
            context = context,
            playerDispatcher = dispatcher,
            authTokenProvider = loginManager.loginState
                .filterIsInstance<LoginState.LoggedIn>()
                .map { it.authToken },
        )
    }

    @Provides
    @IntoSet
    fun providesPlayerLogoutOperation(op: DisposePlayerLogoutOperation): LogoutOperation = op
}