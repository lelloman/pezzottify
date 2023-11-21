package com.lelloman.pezzottify.remoteapi.internal

import com.google.gson.Gson
import okhttp3.Interceptor
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.logging.HttpLoggingInterceptor

private typealias OkHttpResponse = okhttp3.Response

internal interface HttpClient {

    fun setAuthToken(authToken: String?)

    fun get(url: String): Response

    fun multipartPost(url: String): MultipartRequest

    fun multipartPut(url: String): MultipartRequest

    fun delete(url: String): Response

    fun <T> jsonPost(url: String, body: T): Response

    fun <T> jsonPut(url: String, body: T): Response

    enum class BodyMethod {
        POST, PUT,
    }

    interface MultipartRequest {

        fun <T> addJsonField(name: String, o: T): MultipartRequest

        fun addFile(name: String, content: ByteArray): MultipartRequest

        fun addFiles(
            fieldName: String, fileNames: List<String>, contents: List<ByteArray>
        ): MultipartRequest

        fun execute(): Response
    }

    interface Response {
        val status: Int

        val isSuccessful: Boolean

        val is4xx: Boolean

        fun consumeStringBody(): String?

        fun consumeBytesBody(): ByteArray?

        fun <T> consumeBody(clazz: Class<T>): T
    }
}

internal class HttpClientImpl(
    private val gson: Gson,
) : HttpClient {

    private var authToken: String? = null

    private val okHttpClient = OkHttpClient.Builder().followRedirects(false).cache(null)
        .addInterceptor(Interceptor { chain ->
            val builder = chain.request().newBuilder()
            authToken?.let { builder.addHeader("Authorization", "Bearer $it") }
            chain.proceed(builder.build())
        }).addInterceptor(HttpLoggingInterceptor().apply {
            level = HttpLoggingInterceptor.Level.HEADERS
        }).build()

    override fun setAuthToken(authToken: String?) {
        this.authToken = authToken
    }

    override fun get(url: String): HttpClient.Response {
        val request = Request.Builder().get().url(url).build()
        return ResponseSpec(okHttpClient.newCall(request).execute())
    }

    override fun multipartPost(url: String): HttpClient.MultipartRequest =
        MultipartRequestImpl(url, HttpClient.BodyMethod.POST)

    override fun multipartPut(url: String): HttpClient.MultipartRequest =
        MultipartRequestImpl(url, HttpClient.BodyMethod.PUT)

    override fun <T> jsonPost(url: String, body: T): HttpClient.Response =
        doJson(url, gson.toJson(body), HttpClient.BodyMethod.POST)

    override fun <T> jsonPut(url: String, body: T): HttpClient.Response =
        doJson(url, gson.toJson(body), HttpClient.BodyMethod.PUT)

    private fun doJson(url: String, json: String, method: HttpClient.BodyMethod): ResponseSpec {
        return doBodyRequest(url, json.toRequestBody("application/json".toMediaType()), method)
    }

    private fun doBodyRequest(
        url: String, requestBody: RequestBody, method: HttpClient.BodyMethod
    ): ResponseSpec {
        val builder = when (method) {
            HttpClient.BodyMethod.POST -> Request.Builder().post(requestBody)
            HttpClient.BodyMethod.PUT -> Request.Builder().put(requestBody)
        }
        val request = builder.url(url).build()
        return ResponseSpec(okHttpClient.newCall(request).execute())
    }

    override fun delete(url: String) =
        Request.Builder().delete().url(url).build().let(okHttpClient::newCall).execute()
            .let(::ResponseSpec)

    inner class MultipartRequestImpl(
        private val url: String,
        private val method: HttpClient.BodyMethod,
    ) : HttpClient.MultipartRequest {
        private val builder = MultipartBody.Builder().setType(MultipartBody.FORM)

        override fun <T> addJsonField(name: String, o: T) = apply {
            val jsonString = gson.toJson(o)
            val body = jsonString.toRequestBody("application/json".toMediaType())
            builder.addFormDataPart(name, null, body)
        }

        override fun addFile(name: String, content: ByteArray) = apply {
            builder.addFormDataPart(name, name, content.toRequestBody())
        }

        override fun addFiles(
            fieldName: String, fileNames: List<String>, contents: List<ByteArray>
        ) = apply {
            fileNames.forEachIndexed { i, fileName ->
                builder.addFormDataPart(fieldName, fileName, contents[i].toRequestBody())
            }
        }

        override fun execute(): ResponseSpec {
            val body = builder.build()
            return this@HttpClientImpl.doBodyRequest(url, body, method)
        }
    }

    inner class ResponseSpec(private val response: OkHttpResponse) : HttpClient.Response {
        private var bodyString: String? = null
            get() {
                if (field == null) {
                    field = response.body?.string()
                }
                return field
            }

        override val status = response.code

        override val isSuccessful = status in 200..299

        override val is4xx = status in 400..499

        override fun consumeStringBody(): String? = this.bodyString

        override fun consumeBytesBody(): ByteArray? = response.body?.bytes()

        override fun <T> consumeBody(clazz: Class<T>): T = gson.fromJson(bodyString, clazz)
    }
}