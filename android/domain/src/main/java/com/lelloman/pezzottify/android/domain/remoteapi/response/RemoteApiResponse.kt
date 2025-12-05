package com.lelloman.pezzottify.android.domain.remoteapi.response

sealed interface RemoteApiResponse<out T> {

    data class Success<T>(val data: T) : RemoteApiResponse<T>

    sealed interface Error : RemoteApiResponse<Nothing> {
        data object Network : Error

        data object Unauthorized : Error

        data object NotFound : Error

        /**
         * Returned when requesting sync events for a sequence that has been pruned.
         * The client should perform a full sync instead.
         */
        data object EventsPruned : Error

        data class Unknown(val message: String) : Error
    }
}