package com.lelloman.pezzottify.android.notifications

import com.lelloman.pezzottify.android.domain.notifications.SystemNotificationHelper
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class NotificationsModule {

    @Binds
    @Singleton
    abstract fun bindSystemNotificationHelper(
        impl: AndroidSystemNotificationHelper
    ): SystemNotificationHelper
}
