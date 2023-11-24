package com.lelloman.pezzottify.android.app

import com.google.common.truth.Truth.assertThat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.FlowCollector
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.onCompletion
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import org.junit.Ignore
import org.junit.Test
import java.util.concurrent.Executors
import kotlin.coroutines.CoroutineContext

private val threadName get() = Thread.currentThread().name

class TestObserver<T>(private val flow: Flow<T>) :
    FlowCollector<T> {

    private val values = mutableListOf<T>()
    private var completed = false

    suspend fun start(context: CoroutineContext = Dispatchers.Default) = apply {
        withContext(context) {
            flow.onCompletion { completed = true }
                .collect(this@TestObserver)
        }
    }

    override suspend fun emit(value: T) {
        println("TestObserver collecting value $value on $threadName")
        values.add(value)
    }

    fun assertValuesCount(count: Int) = apply {
        assertThat(values).hasSize(count)
    }

    fun assertValues(vararg v: T) = apply {
        assertThat(values).isEqualTo(v.toList())
    }

    fun assertEmpty() = apply { assertThat(values).isEmpty() }

    fun assertCompleted() = apply { assertThat(completed).isTrue() }

    fun assertNotCompleted() = apply { assertThat(completed).isFalse() }
}

fun <T> Flow<T>.test() =
    TestObserver(this)

@Ignore("Just to understand flows")
class FlowTests {

    @Test
    fun `just a flow`() {
        runBlocking {
            val flow = flowOf(1, 2, 3)

            val tester = flow.test().start()

            tester.assertValuesCount(3)
                .assertCompleted()

            flow
                .map { it * 2 }
                .test()
                .start()
                .assertValues(2, 4, 6)
                .assertCompleted()
        }
    }

    @Test
    fun flowy() {
        println("start test function on $threadName")
        val flow = flow {
            println("inside flow on $threadName")
            delay(100)
            emit(1)
            delay(500)
            emit(2)
            delay(1000)
            emit(3)
        }

        val executor1 = Executors.newSingleThreadExecutor().asCoroutineDispatcher()
        val executor2 = Executors.newSingleThreadExecutor().asCoroutineDispatcher()

        // the collection happens asynchronously, it'll complete after all the delay in the flow
        val tester1 = flow.flowOn(executor1).test()
        CoroutineScope(executor2).launch { tester1.start() }
        tester1.assertEmpty()
            .assertNotCompleted()

        // same as before, but joining the job means tester will complete
        val tester2 = flow.flowOn(executor1).test()
        val job = CoroutineScope(executor2).launch { tester2.start() }
        runBlocking {
            job.join()
            tester1.assertCompleted()
        }

        // all in runBlocking, doesn't matter what executor it flows or collect, we're still waiting
        runBlocking {
            flow.flowOn(executor1)
                .test()
                .start(executor2)
                .assertValues(1, 2, 3)
                .assertCompleted()
        }

        val tester3 = flow.test()
        runBlocking {
            tester3.start().assertCompleted()
        }
    }
}
