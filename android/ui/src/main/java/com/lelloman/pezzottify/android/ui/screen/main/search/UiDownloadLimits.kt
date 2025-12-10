package com.lelloman.pezzottify.android.ui.screen.main.search

data class UiDownloadLimits(
    val requestsToday: Int,
    val maxPerDay: Int,
    val canRequest: Boolean,
    val inQueue: Int,
    val maxQueue: Int,
) {
    val isAtDailyLimit: Boolean get() = requestsToday >= maxPerDay
    val isAtQueueLimit: Boolean get() = inQueue >= maxQueue
    val isAtAnyLimit: Boolean get() = isAtDailyLimit || isAtQueueLimit
}
