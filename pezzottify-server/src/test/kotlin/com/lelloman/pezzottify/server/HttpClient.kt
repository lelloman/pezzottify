package com.lelloman.pezzottify.server

import com.google.gson.Gson
import com.google.gson.GsonBuilder
import okhttp3.*
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.boot.test.web.server.LocalServerPort
import org.springframework.http.codec.json.Jackson2JsonDecoder
import org.springframework.http.converter.json.Jackson2ObjectMapperBuilder

class HttpClient(private val baseUrl: String) {
    private var cookiesEnabled = true

    val gson = GsonBuilder().create()

    private inner class Cookies : CookieJar {
        private val stored = mutableListOf<Cookie>()

        override fun loadForRequest(url: HttpUrl): List<Cookie> {
            return if (cookiesEnabled) stored else emptyList()
        }

        override fun saveFromResponse(url: HttpUrl, cookies: List<Cookie>) {
            if (cookiesEnabled)
                stored.addAll(cookies)
        }
    }

    inner class ResponseSpec(private val response: Response) {
        var bodyString: String? = null
            get() {
                if (field == null) {
                    field = response.body?.string()
                }
                return field
            }

        fun assertStatus(code: Int): ResponseSpec = apply {
            assertThat(this.response.code).isEqualTo(code)
        }

        fun bodyString(consumer: (String?) -> Unit): ResponseSpec = apply {
            consumer(this.bodyString)
        }

        fun assertRedirectTo(to: String): ResponseSpec = apply {
            assertThat(response.isRedirect).isTrue()
            val expectedLocation = "$baseUrl$to"
            assertThat(response.headers["Location"]).isEqualTo(expectedLocation)
        }

        fun assertRedirectTo(action: (String) -> Unit) = apply {
            assertThat(response.isRedirect).isTrue()
            val redirect = response.headers["Location"]
            assertThat(redirect).isNotNull()
            action(redirect!!)
        }

        inline fun <reified T> parsedBody(action: (T) -> Unit): ResponseSpec = apply {
            action(gson.fromJson(this.bodyString, T::class.java))
        }
    }

    inner class FormPostRequest(private val httpClient: HttpClient, private val url: String) {
        private val formBuilder = FormBody.Builder()

        fun add(name: String, value: String) = apply {
            formBuilder.addEncoded(name, value)
        }

        fun execute(): ResponseSpec {
            return httpClient.doPost(url, formBuilder)
        }
    }

    private val cookieJar = Cookies()
    private val okHttpClient = OkHttpClient.Builder()
        .followRedirects(false)
        .cookieJar(cookieJar)
        .build()

    fun get(url: String): ResponseSpec {
        val url = "$baseUrl$url"
        val request = Request.Builder().get().url(url).build()
        return ResponseSpec(okHttpClient.newCall(request).execute())
    }

    private fun doPost(url: String, formBuilder: FormBody.Builder): ResponseSpec {
        val url = "$baseUrl$url"
        val request = Request.Builder().post(formBuilder.build()).url(url).build()
        return ResponseSpec(okHttpClient.newCall(request).execute())
    }

    fun formPost(url: String): FormPostRequest {
        return FormPostRequest(this, url)
    }

    private fun disableCookies() {
        this.cookiesEnabled = false
    }

    private fun enableCookies() {
        this.cookiesEnabled = true
    }

    fun withoutCookies(action: HttpClient.() -> Unit) {
        disableCookies()
        action(this)
        enableCookies()
    }
}