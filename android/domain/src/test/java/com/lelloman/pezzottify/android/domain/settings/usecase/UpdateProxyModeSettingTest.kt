package com.lelloman.pezzottify.android.domain.settings.usecase

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import com.lelloman.pezzottify.android.domain.usercontent.SyncStatus
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.mockk
import kotlinx.coroutines.test.runTest
import org.junit.Test

class UpdateProxyModeSettingTest {

    private val userSettingsStore = mockk<UserSettingsStore>(relaxed = true)
    private val remoteApiClient = mockk<RemoteApiClient>()
    private val updateProxyModeSetting = UpdateProxyModeSetting(userSettingsStore, remoteApiClient)

    @Test
    fun `updates local state only after server accepts the setting`() = runTest {
        val setting = UserSetting.ProxyModeEnabled(true)
        coEvery { remoteApiClient.updateUserSettings(listOf(setting)) } returns
            RemoteApiResponse.Success(Unit)

        assertThat(updateProxyModeSetting(true)).isTrue()

        coVerify(ordering = io.mockk.Ordering.SEQUENCE) {
            remoteApiClient.updateUserSettings(listOf(setting))
            userSettingsStore.setSyncedSetting(setting, SyncStatus.Synced)
        }
    }

    @Test
    fun `keeps local state unchanged when server rejects the setting`() = runTest {
        val setting = UserSetting.ProxyModeEnabled(true)
        coEvery { remoteApiClient.updateUserSettings(listOf(setting)) } returns
            RemoteApiResponse.Error.Network

        assertThat(updateProxyModeSetting(true)).isFalse()

        coVerify(exactly = 0) { userSettingsStore.setSyncedSetting(any(), any()) }
    }
}
