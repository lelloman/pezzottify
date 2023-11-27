package com.lelloman.pezzottify.android.log

import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton
import kotlin.reflect.KProperty

@Module
@InstallIn(SingletonComponent::class)
class LogModule {

    @Singleton
    @Provides
    fun provideLoggerFactory(): LoggerFactory = object : LoggerFactory {
        override fun getValue(obj: Any, property: KProperty<*>): Logger =
            LogcatLogger(obj.javaClass.simpleName)
    }
}