package com.lelloman.pezzottify.android.app.di

import com.google.gson.GsonBuilder
import com.lelloman.pezzottify.android.app.domain.login.LoginManagerImpl
import com.lelloman.pezzottify.android.app.localdata.PersistentObjectDef
import com.lelloman.pezzottify.android.app.ui.login.LoginViewModel
import com.lelloman.pezzottify.remoteapi.RemoteApi
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import javax.inject.Qualifier
import javax.inject.Singleton

@Qualifier
@Retention(AnnotationRetention.RUNTIME)
annotation class IoDispatcher

@Module
@InstallIn(SingletonComponent::class)
class ApplicationModule {

    @Provides
    @Singleton
    fun provideGson() = GsonBuilder()

    @Provides
    @IoDispatcher
    fun provideIoDispatcher(): CoroutineDispatcher = Dispatchers.IO

    @Provides
    @IntoSet
    fun provideLoginPersistentObjectDef(): PersistentObjectDef<*> =
        LoginViewModel.PersistentObject.def

    @Provides
    @IntoSet
    fun provideLoggedInPersistentObjectDef(): PersistentObjectDef<*> =
        LoginManagerImpl.persistenceObjectDef

    @Provides
    @Singleton
    fun provideRemoteApi(@IoDispatcher ioDispatcher: CoroutineDispatcher) =
        RemoteApi.Factory.create(ioDispatcher)
}