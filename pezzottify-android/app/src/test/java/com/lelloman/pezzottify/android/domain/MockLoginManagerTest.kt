package com.lelloman.pezzottify.android.domain

import com.google.common.truth.Truth.assertThat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.InternalCoroutinesApi
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.mockito.kotlin.mock
import org.mockito.kotlin.whenever

class MockLoginManagerTest {

    private val interactor: RemoteLoginInteractor = mock()
    private val tested = MockLoginManager(interactor)

    private suspend fun loginFail() {
        delay(5000)
        throw Exception("something networky")
    }

    private suspend fun assertLogin() {
        val result = tested.performLogin("", "")
        assertThat(result).isEqualTo(LoginResult.Success("asd"))
    }

    private fun assertFailure() {
        assertThat("").isNull()
    }

    @OptIn(InternalCoroutinesApi::class)
    @Test
    fun asd() = runBlocking {
        whenever(interactor.doLogin()).thenAnswer {
            runBlocking {
                //CoroutineScope(Dispatchers.IO).launch {
                delay(5000)
                //throw Exception("networky thing")
                //}.join()
            }
        }
        CoroutineScope(Dispatchers.IO).launch {
            assertLogin()
        }
//        assertFailure()
    }
}