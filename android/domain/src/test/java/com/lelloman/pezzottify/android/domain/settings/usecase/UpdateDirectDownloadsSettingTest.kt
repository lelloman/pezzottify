package com.lelloman.pezzottify.android.domain.settings.usecase

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.domain.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.domain.remoteapi.response.RemoteApiResponse
import com.lelloman.pezzottify.android.domain.settings.UserSettingsStore
import com.lelloman.pezzottify.android.domain.sync.UserSetting
import io.mockk.coEvery
import io.mockk.coVerify
import io.mockk.coVerifyOrder
import io.mockk.every
import io.mockk.mockk
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runTest
import org.junit.Before
import org.junit.Test

class UpdateDirectDownloadsSettingTest {

    private lateinit var remoteApiClient: RemoteApiClient
    private lateinit var userSettingsStore: UserSettingsStore
    private lateinit var useCase: UpdateDirectDownloadsSetting

    private val directDownloadsEnabledFlow = MutableStateFlow(false)

    @Before
    fun setUp() {
        remoteApiClient = mockk(relaxed = true)
        userSettingsStore = mockk(relaxed = true) {
            every { directDownloadsEnabled } returns directDownloadsEnabledFlow
        }
        useCase = UpdateDirectDownloadsSetting(remoteApiClient, userSettingsStore)
    }

    @Test
    fun `invoke returns Success when server request succeeds`() = runTest {
        coEvery { remoteApiClient.updateUserSettings(any()) } returns RemoteApiResponse.Success(Unit)

        val result = useCase(enabled = true)

        assertThat(result).isEqualTo(UpdateDirectDownloadsSetting.Result.Success)
    }

    @Test
    fun `invoke updates local store optimistically`() = runTest {
        coEvery { remoteApiClient.updateUserSettings(any()) } returns RemoteApiResponse.Success(Unit)

        useCase(enabled = true)

        coVerify { userSettingsStore.setDirectDownloadsEnabled(true) }
    }

    @Test
    fun `invoke sends correct setting to server`() = runTest {
        coEvery { remoteApiClient.updateUserSettings(any()) } returns RemoteApiResponse.Success(Unit)

        useCase(enabled = true)

        coVerify {
            remoteApiClient.updateUserSettings(listOf(UserSetting.DirectDownloadsEnabled(true)))
        }
    }

    @Test
    fun `invoke returns Error when server request fails`() = runTest {
        coEvery { remoteApiClient.updateUserSettings(any()) } returns RemoteApiResponse.Error.Network

        val result = useCase(enabled = true)

        assertThat(result).isEqualTo(UpdateDirectDownloadsSetting.Result.Error)
    }

    @Test
    fun `invoke reverts local store on failure`() = runTest {
        directDownloadsEnabledFlow.value = false
        coEvery { remoteApiClient.updateUserSettings(any()) } returns RemoteApiResponse.Error.Network

        useCase(enabled = true)

        coVerifyOrder {
            userSettingsStore.setDirectDownloadsEnabled(true)  // Optimistic update
            userSettingsStore.setDirectDownloadsEnabled(false) // Revert
        }
    }

    @Test
    fun `invoke disabling setting works correctly`() = runTest {
        directDownloadsEnabledFlow.value = true
        coEvery { remoteApiClient.updateUserSettings(any()) } returns RemoteApiResponse.Success(Unit)

        val result = useCase(enabled = false)

        assertThat(result).isEqualTo(UpdateDirectDownloadsSetting.Result.Success)
        coVerify {
            remoteApiClient.updateUserSettings(listOf(UserSetting.DirectDownloadsEnabled(false)))
        }
    }
}
