package com.lelloman.pezzottify.android.app.localdata

import com.lelloman.pezzottify.android.app.di.IoDispatcher
import com.lelloman.pezzottify.android.app.domain.LoginOperation
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.CoroutineDispatcher

@Module
@InstallIn(SingletonComponent::class)
class Module {
    @Provides
    fun provideStaticFetcherOnLogin(@IoDispatcher ioDispatcher: CoroutineDispatcher) =
        StaticFetcherOnLogin(ioDispatcher)

    @Provides
    @IntoSet
    fun provideLoginOperation(staticFetcherOnLogin: StaticFetcherOnLogin): LoginOperation =
        staticFetcherOnLogin
}