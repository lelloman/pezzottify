package com.lelloman.pezzottify.android.remoteapi.internal

import com.google.common.truth.Truth.assertThat
import com.lelloman.pezzottify.android.remoteapi.RemoteApiClient
import com.lelloman.pezzottify.android.remoteapi.RemoteApiCredentialsProvider
import com.lelloman.pezzottify.android.remoteapi.response.RemoteApiResponse
import kotlinx.coroutines.test.runTest
import org.junit.Test

class RemoteApiClientImplTest {

    @Test
    fun `smoke test with actual backend`() = runTest {
        // Setup
        val userHandle = "android-test"
        val password = "asdasd"
        val baseUrl = "http://localhost:3001"
        val credentialsProvider = object : RemoteApiCredentialsProvider {
            override var authToken: String = ""
        }
        val client = RemoteApiClient.Factory.create(
            baseUrl = baseUrl,
            credentialsProvider = credentialsProvider,
        )

        // Can't get artist without auth token
        val princeId = "R5a2EaR3hamoenG9rDuVn8j"
        val forbiddenResponse = client.getArtist(princeId)
        assertThat(forbiddenResponse).isEqualTo(RemoteApiResponse.Error.Unauthorized)

        // The login endpoint returns the token
        val loginResponse = client.login(userHandle, password)
        assertThat(loginResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        credentialsProvider.authToken = (loginResponse as RemoteApiResponse.Success).data.token

        // Here we can get the artist
        val princeResponse = client.getArtist(princeId)
        assertThat(princeResponse).isInstanceOf(RemoteApiResponse.Success::class.java)
        val prince = (princeResponse as RemoteApiResponse.Success).data
        assertThat(prince.name).isEqualTo("Prince")


    }
}