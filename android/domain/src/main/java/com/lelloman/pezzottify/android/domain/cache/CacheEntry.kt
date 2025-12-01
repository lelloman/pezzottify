package com.lelloman.pezzottify.android.domain.cache

data class CacheEntry<T>(
    val value: T,
    val createdAt: Long,
    val lastAccessedAt: Long,
    val sizeBytes: Int
) {
    fun isExpired(ttlMillis: Long, currentTime: Long): Boolean {
        return (currentTime - createdAt) > ttlMillis
    }

    fun touch(currentTime: Long): CacheEntry<T> {
        return copy(lastAccessedAt = currentTime)
    }
}
