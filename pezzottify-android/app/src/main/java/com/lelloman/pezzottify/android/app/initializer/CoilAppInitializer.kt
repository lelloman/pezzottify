package com.lelloman.pezzottify.android.app.initializer

import coil.Coil
import coil.ImageLoader
import com.lelloman.pezzottify.android.app.PezzottifyApp
import com.lelloman.pezzottify.android.app.domain.login.LoginManager
import okhttp3.OkHttpClient

class CoilAppInitializer(private val loginManager: LoginManager) : AppInitializer {
    override fun init(app: PezzottifyApp) {
        Coil.setImageLoader {
            ImageLoader.Builder(app)
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