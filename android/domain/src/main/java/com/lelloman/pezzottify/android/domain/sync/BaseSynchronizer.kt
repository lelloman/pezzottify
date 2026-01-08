package com.lelloman.pezzottify.android.domain.sync

import com.lelloman.pezzottify.android.domain.app.AppInitializer
import com.lelloman.pezzottify.android.logger.Logger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds

/**
 * Base class for background synchronizers that process items in a loop with exponential backoff.
 *
 * Subclasses must implement:
 * - [getItemsToProcess]: Returns items that need to be processed
 * - [processItem]: Processes a single item
 *
 * Optional overrides:
 * - [shouldContinueWhenNoItems]: Return true to keep looping even when no items (default: false)
 * - [onBeforeMainLoop]: Hook called once before the main loop starts
 *
 * The synchronizer sleeps when there are no items to process and can be woken up via [wakeUp].
 * Sleep duration increases exponentially between iterations (up to [maxSleepDuration]) and
 * resets to [minSleepDuration] when woken up.
 */
abstract class BaseSynchronizer<T>(
    protected val logger: Logger,
    private val dispatcher: CoroutineDispatcher,
    private val scope: CoroutineScope,
    private val minSleepDuration: Duration = DEFAULT_MIN_SLEEP_DURATION,
    private val maxSleepDuration: Duration = DEFAULT_MAX_SLEEP_DURATION,
) : AppInitializer {

    private var initialized = false
    private var sleepDuration = minSleepDuration
    private var wakeUpSignal = CompletableDeferred<Unit>()

    /**
     * Returns the list of items that need to be processed.
     * Called at the start of each loop iteration.
     */
    protected abstract suspend fun getItemsToProcess(): List<T>

    /**
     * Processes a single item. Called for each item returned by [getItemsToProcess].
     */
    protected abstract suspend fun processItem(item: T)

    /**
     * Processes all items from a single iteration. Override to implement batch processing.
     * Default implementation processes items one-by-one via [processItem].
     */
    protected open suspend fun processItems(items: List<T>) {
        items.forEach { processItem(it) }
    }

    /**
     * Returns true if the loop should continue even when [getItemsToProcess] returns empty.
     * Default is false (go to sleep when no items).
     *
     * Useful when there may be items in a "loading" state that will become available later.
     */
    protected open suspend fun shouldContinueWhenNoItems(): Boolean = false

    /**
     * Hook called once before the main loop starts.
     * Override to perform initialization tasks like resetting state.
     */
    protected open suspend fun onBeforeMainLoop() {}

    override fun initialize() {
        scope.launch(dispatcher) {
            if (!initialized) {
                initialized = true
                onBeforeMainLoop()
                mainLoop()
            }
        }
    }

    /**
     * Wakes up the synchronizer if it's sleeping, causing it to immediately
     * check for new items to process. Also resets the sleep duration to minimum.
     */
    fun wakeUp() {
        logger.info("wakeUp()")
        wakeUpSignal.complete(Unit)
        sleepDuration = minSleepDuration
    }

    private suspend fun mainLoop() {
        while (true) {
            logger.debug("mainLoop() iteration")
            val itemsToProcess = getItemsToProcess()
            logger.debug("mainLoop() got ${itemsToProcess.size} items to process")

            if (itemsToProcess.isEmpty() && !shouldContinueWhenNoItems()) {
                logger.info("mainLoop() going to sleep")
                wakeUpSignal.await()
                wakeUpSignal = CompletableDeferred()
                sleepDuration = minSleepDuration
                continue
            }

            processItems(itemsToProcess)

            logger.debug("mainLoop() going to wait for $sleepDuration")
            delay(sleepDuration)
            sleepDuration *= BACKOFF_MULTIPLIER
            if (sleepDuration > maxSleepDuration) {
                sleepDuration = maxSleepDuration
            }
        }
    }

    companion object {
        private val DEFAULT_MIN_SLEEP_DURATION = 10.milliseconds
        private val DEFAULT_MAX_SLEEP_DURATION = 10.seconds
        private const val BACKOFF_MULTIPLIER = 1.4
    }
}
