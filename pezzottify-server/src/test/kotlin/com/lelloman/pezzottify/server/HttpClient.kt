package com.lelloman.pezzottify.server

import com.google.gson.GsonBuilder
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import org.assertj.core.api.Assertions.assertThat

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

        inline fun <reified T> parsedBody(): T = gson.fromJson(this.bodyString, T::class.java)
    }

    inner class FormPostRequest(private val httpClient: HttpClient, private val url: String) {
        private val formBuilder = FormBody.Builder()

        fun add(name: String, value: String) = apply {
            formBuilder.addEncoded(name, value)
        }

        fun execute(): ResponseSpec {
            return httpClient.doPost(url, formBuilder.build())
        }
    }

    inner class MultipartPostRequest(private val httpClient: HttpClient, private val url: String) {
        private val builder = MultipartBody.Builder().setType(MultipartBody.FORM)

        fun <T> addJsonField(name: String, o: T) = apply {
            val jsonString = gson.toJson(o)
            val body = jsonString.toRequestBody("application/json".toMediaType())
            builder.addFormDataPart(name, null, body)
        }

        fun addFile(name: String, content: ByteArray) = apply {
            builder.addFormDataPart(name, name, content.toRequestBody())
        }

        fun execute(): ResponseSpec {
            val body = builder.build()
            return httpClient.doPost(url, body)
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

    private fun doPost(url: String, requestBody: RequestBody): ResponseSpec {
        val url = "$baseUrl$url"
        val request = Request.Builder().post(requestBody).url(url).build()
        return ResponseSpec(okHttpClient.newCall(request).execute())
    }

    private fun doPost(url: String, body: String): ResponseSpec {
        val url = "$baseUrl$url"
        val body = body.toRequestBody("application/json".toMediaType())
        val request = Request.Builder().post(body).url(url).build()
        return ResponseSpec(okHttpClient.newCall(request).execute())
    }

    fun formPost(url: String): FormPostRequest {
        return FormPostRequest(this, url)
    }

    fun multipartPost(url: String): MultipartPostRequest {
        return MultipartPostRequest(this, url)
    }

    fun performAdminLogin() {
        formPost("/login")
            .add("username", "admin")
            .add("password", "admin")
            .execute()
            .assertRedirectTo("/")
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