package com.lelloman.pezzottify.android.domain.settings.usecase

import com.lelloman.pezzottify.android.domain.settings.UserSettingsSynchronizer
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import io.mockk.coVerify
import io.mockk.mockk
import io.mockk.verify
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class UpdateDirectDownloadsSettingTest {

    private lateinit var userSettingsStore: UserSettingsStore
    private lateinit var userSettingsSynchronizer: UserSettingsSynchronizer
    private lateinit var useCase: UpdateDirectDownloadsSetting

    @Before
    fun setUp() {
        userSettingsStore = mockk(relaxed = true)
        userSettingsSynchronizer = mockk(relaxed = true)
        useCase = UpdateDirectDownloadsSetting(userSettingsStore, userSettingsSynchronizer)
    }

    @Test
    fun `invoke saves setting locally with PendingSync status`() = runTest {
        useCase(enabled = true)

        coVerify {
            userSettingsStore.setSyncedSetting(
                setting = UserSetting.DirectDownloadsEnabled(true),
                syncStatus = SyncStatus.PendingSync,
            )
        }
    }

    @Test
    fun `invoke wakes up the synchronizer`() = runTest {
        useCase(enabled = true)

        verify { userSettingsSynchronizer.wakeUp() }
    }

    @Test
    fun `invoke with false saves disabled setting`() = runTest {
        useCase(enabled = false)

        coVerify {
            userSettingsStore.setSyncedSetting(
                setting = UserSetting.DirectDownloadsEnabled(false),
                syncStatus = SyncStatus.PendingSync,
            )
        }
    }
}
