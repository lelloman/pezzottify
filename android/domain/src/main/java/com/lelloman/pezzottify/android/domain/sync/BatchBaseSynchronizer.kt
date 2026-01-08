package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/**
 * A [BaseSynchronizer] variant that processes items in batches instead of one-by-one.
 *
 * Subclasses must implement [processBatch] instead of [processItem].
 */
abstract class BatchBaseSynchronizer<T>(
    logger: Logger,
    dispatcher: CoroutineDispatcher,
    scope: CoroutineScope,
    minSleepDuration: Duration = DEFAULT_MIN_SLEEP_DURATION,
    maxSleepDuration: Duration = DEFAULT_MAX_SLEEP_DURATION,
) : BaseSynchronizer<T>(
    logger = logger,
    dispatcher = dispatcher,
    scope = scope,
    minSleepDuration = minSleepDuration,
    maxSleepDuration = maxSleepDuration,
) {

    final override suspend fun processItem(item: T) {
        error("processItem() should not be called on BatchBaseSynchronizer")
    }

    final override suspend fun processItems(items: List<T>) {
        processBatch(items)
    }

    /**
     * Processes a batch of items. Called once per iteration with all items
     * returned by [getItemsToProcess].
     */
    protected abstract suspend fun processBatch(items: List<T>)

    companion object {
        private val DEFAULT_MIN_SLEEP_DURATION = 10.milliseconds
        private val DEFAULT_MAX_SLEEP_DURATION = 10.seconds
    }
}
