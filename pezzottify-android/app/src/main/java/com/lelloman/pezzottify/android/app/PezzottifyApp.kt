package com.lelloman.pezzottify.android.app

import android.app.Application
import coil.Coil
import coil.ImageLoader
import com.lelloman.pezzottify.android.app.domain.LoginManager
import com.lelloman.pezzottify.android.app.domain.LoginStateOperationsCollector
import dagger.hilt.android.HiltAndroidApp
import okhttp3.OkHttpClient
import javax.inject.Inject

@HiltAndroidApp
class PezzottifyApp : Application() {

    @Inject
    lateinit var loginStateOperationsCollector: LoginStateOperationsCollector

    @Inject
    lateinit var loginManager: LoginManager

    override fun onCreate() {
        super.onCreate()
        loginStateOperationsCollector.register(loginManager)

        Coil.setImageLoader {
            ImageLoader.Builder(this)
                .okHttpClient {
                    OkHttpClient.Builder()
                        .addInterceptor { chain ->
                            val newRequestBuilder = chain.request().newBuilder()
                            newRequestBuilder.addHeader(
                                "Authorization",
                                "Bearer ${loginManager.getAuthToken()}"
                            )
                            chain.proceed(newRequestBuilder.build())
                        }
                        .build()
                }
                .build()
        }
    }
}