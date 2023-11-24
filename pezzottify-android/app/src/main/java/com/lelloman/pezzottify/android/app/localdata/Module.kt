package com.lelloman.pezzottify.android.app.localdata

import com.lelloman.pezzottify.android.app.di.IoDispatcher
import com.lelloman.pezzottify.android.app.domain.LoginOperation
import com.lelloman.pezzottify.android.app.domain.LogoutOperation
import com.lelloman.pezzottify.android.localdata.LocalDb
import com.lelloman.pezzottify.android.localdata.StaticsDao
import com.lelloman.pezzottify.remoteapi.RemoteApi
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
    fun provideStaticFetcherOnLogin(
        @IoDispatcher ioDispatcher: CoroutineDispatcher,
        remoteApi: RemoteApi,
        staticsDao: StaticsDao,
    ) = FetchStaticsLoginOperation(
        remoteApi = remoteApi,
        dispatcher = ioDispatcher,
        staticsDao = staticsDao,
    )

    @Provides
    fun provideDeleteStaticsLogoutOperation(localDb: LocalDb) =
        DeleteStaticsLogoutOperation(localDb)

    @Provides
    @IntoSet
    fun provideLoginOperation(fetchStaticsLoginOperation: FetchStaticsLoginOperation): LoginOperation =
        fetchStaticsLoginOperation

    @Provides
    @IntoSet
    fun provideLogoutOperation(deleteStaticsLogoutOperation: DeleteStaticsLogoutOperation): LogoutOperation =
        deleteStaticsLogoutOperation
}