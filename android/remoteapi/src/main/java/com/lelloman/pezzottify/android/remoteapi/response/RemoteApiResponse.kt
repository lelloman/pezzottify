package com.lelloman.pezzottify.android.remoteapi.response

sealed interface RemoteApiResponse<out T> {

    data class Success<T>(val data: T) : RemoteApiResponse<T>

    sealed interface Error : RemoteApiResponse<Nothing> {
        data object Network : Error

        data object Unauthorized : Error

        data object NotFound : Error

        data class Unknown(val message: String) : Error
    }
}