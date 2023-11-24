package com.lelloman.pezzottify.android.app.di

import com.google.gson.GsonBuilder
import com.lelloman.pezzottify.android.app.persistence.PersistentObjectDef
import com.lelloman.pezzottify.android.app.ui.login.LoginViewModel
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import dagger.multibindings.IntoSet
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import javax.inject.Qualifier

@Qualifier
@Retention(AnnotationRetention.RUNTIME)
annotation class IoDispatcher

@Module
@InstallIn(SingletonComponent::class)
class ApplicationModule {

    @Provides
    fun provideGson() = GsonBuilder()

    @Provides
    @IoDispatcher
    fun provideIoDispatcher(): CoroutineDispatcher = Dispatchers.IO

    @Provides
    @IntoSet
    fun provideLoginPersistentObjectDef(): PersistentObjectDef<*> =
        LoginViewModel.PersistentObject.def
}