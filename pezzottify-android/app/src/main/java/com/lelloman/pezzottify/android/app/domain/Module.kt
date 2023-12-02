package com.lelloman.pezzottify.android.app.domain

import android.content.Context
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import com.lelloman.pezzottify.android.app.domain.login.LoginManagerImpl
import com.lelloman.pezzottify.android.app.domain.login.LoginOperation
import com.lelloman.pezzottify.android.app.domain.login.LoginStateOperationsCollector
import com.lelloman.pezzottify.android.app.domain.login.LogoutOperation
import com.lelloman.pezzottify.android.app.domain.statics.DeleteStaticsLogoutOperation
import com.lelloman.pezzottify.android.app.domain.statics.FetchStaticsLoginOperation
import com.lelloman.pezzottify.android.app.localdata.ObjectsStore
import com.lelloman.pezzottify.remoteapi.RemoteApi
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.Dispatchers
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
class Module {

    @Provides
    @Singleton
    fun provideLoginManager(
        @ApplicationContext context: Context,
        remoteApi: RemoteApi,
        objectsStore: ObjectsStore,
    ): LoginManager = LoginManagerImpl(
        remoteApi = remoteApi,
        objectsStore = objectsStore,
        ioDispatcher = Dispatchers.IO,
    )

    @Provides
    @Singleton
    fun provideLoginStateOperationsCollector(
        loginOperations: Set<@JvmSuppressWildcards LoginOperation>,
        logoutOperations: Set<@JvmSuppressWildcards LogoutOperation>,
    ) = LoginStateOperationsCollector(loginOperations, logoutOperations)

    @Provides
    @IntoSet
    fun provideLoginOperation(fetchStaticsLoginOperation: FetchStaticsLoginOperation): LoginOperation =
        fetchStaticsLoginOperation

    @Provides
    @IntoSet
    fun provideLogoutOperation(deleteStaticsLogoutOperation: DeleteStaticsLogoutOperation): LogoutOperation =
        deleteStaticsLogoutOperation
}