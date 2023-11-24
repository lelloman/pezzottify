package com.lelloman.pezzottify.remoteapi

import com.google.common.truth.Truth.assertThat
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import org.junit.Ignore
import org.junit.Test

@Ignore("Needs running server")
class RemoteApiImplTest {

    private val tested = RemoteApi.create(Dispatchers.IO)
    private val remoteUrl = "http://127.0.0.1:8080"

    private suspend fun performUserLogin() {
        with(tested.performLogin(remoteUrl, "user", "user")) {
            assertThat(this).isInstanceOf(RemoteApi.Response.Success::class.java)
            val loginResponse = (this as RemoteApi.Response.Success).value
            assertThat(loginResponse).isInstanceOf(LoginResponse.Success::class.java)
            assertThat((loginResponse as LoginResponse.Success).authToken).isNotEmpty()
        }
    }

    @Test
    fun `performs admin login`() = runBlocking {
        with(tested.performLogin(remoteUrl, "admin", "wrong pw")) {
            assertThat(this).isInstanceOf(RemoteApi.Response.Success::class.java)
            val loginResponse = (this as RemoteApi.Response.Success).value
            assertThat(loginResponse).isInstanceOf(LoginResponse.InvalidCredentials::class.java)
        }

        with(tested.performLogin(remoteUrl, "admin", "admin")) {
            assertThat(this).isInstanceOf(RemoteApi.Response.Success::class.java)
            val loginResponse = (this as RemoteApi.Response.Success).value
            assertThat(loginResponse).isInstanceOf(LoginResponse.Success::class.java)
            assertThat((loginResponse as LoginResponse.Success).authToken).isNotEmpty()
        }
    }

    @Test
    fun `performs user login`() = runBlocking {
        val tested = RemoteApi.create(Dispatchers.IO)
        with(tested.performLogin(remoteUrl, "user", "wrong pw")) {
            assertThat(this).isInstanceOf(RemoteApi.Response.Success::class.java)
            val loginResponse = (this as RemoteApi.Response.Success).value
            assertThat(loginResponse).isInstanceOf(LoginResponse.InvalidCredentials::class.java)
        }

        performUserLogin()
    }

    @Test
    fun `reads user state`() = runBlocking {
        performUserLogin()

        with(tested.getUserState()) {
            assertThat(this).isInstanceOf(RemoteApi.Response.Success::class.java)
            val userState = (this as RemoteApi.Response.Success).value
            assertThat(userState.bookmarkedAlbums).isEmpty()
            assertThat(userState.playlists).isEmpty()
        }
    }

    @Test
    fun `reads albums`() = runBlocking {
        performUserLogin()

        with(tested.getAlbums()) {
            assertThat(this).isInstanceOf(RemoteApi.Response.Success::class.java)
            val albums = (this as RemoteApi.Response.Success).value
            assertThat(albums).hasSize(2)
        }
    }
}