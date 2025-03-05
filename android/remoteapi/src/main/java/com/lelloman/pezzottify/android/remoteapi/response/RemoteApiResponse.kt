package com.lelloman.pezzottify.android.remoteapi.response

sealed interface RemoteApiResponse<T> {

    data class Success<T>(val data: T) : RemoteApiResponse<T>

    sealed interface Error : RemoteApiResponse<Any> {
        data object Network : Error

        data object Unauthorized : Error

        data object NotFound : Error

        data class Unknown(val message: String) : Error
    }
}