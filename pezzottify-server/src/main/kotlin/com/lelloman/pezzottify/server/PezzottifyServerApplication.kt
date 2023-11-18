package com.lelloman.pezzottify.server

import org.slf4j.Logger
import org.slf4j.LoggerFactory
import org.springframework.aop.interceptor.AsyncUncaughtExceptionHandler
import org.springframework.aop.interceptor.SimpleAsyncUncaughtExceptionHandler
import org.springframework.boot.autoconfigure.EnableAutoConfiguration
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.core.task.AsyncTaskExecutor
import org.springframework.scheduling.annotation.AsyncConfigurer
import org.springframework.scheduling.annotation.EnableAsync
import org.springframework.scheduling.annotation.EnableScheduling
import org.springframework.scheduling.concurrent.ThreadPoolTaskExecutor
import org.springframework.web.context.request.NativeWebRequest
import org.springframework.web.context.request.async.CallableProcessingInterceptor
import org.springframework.web.context.request.async.TimeoutCallableProcessingInterceptor
import org.springframework.web.filter.CommonsRequestLoggingFilter
import org.springframework.web.servlet.config.annotation.AsyncSupportConfigurer
import org.springframework.web.servlet.config.annotation.WebMvcConfigurer
import java.util.concurrent.Callable


@SpringBootApplication
@EnableAutoConfiguration
class PezzottifyServerApplication

fun main(args: Array<String>) {
    runApplication<PezzottifyServerApplication>(*args)
}

@Configuration
class RequestLoggingFilterConfig {
    @Bean
    fun logFilter(): CommonsRequestLoggingFilter {
        val filter = CommonsRequestLoggingFilter()
        filter.setIncludeQueryString(true)
        filter.setIncludePayload(true)
        filter.setMaxPayloadLength(1000)
        filter.setIncludeHeaders(true)
        filter.setAfterMessagePrefix("REQUEST DATA: ")
        return filter
    }
}


@Configuration
@EnableAsync
@EnableScheduling
class AsyncConfiguration : AsyncConfigurer {
    private val log: Logger = LoggerFactory.getLogger(AsyncConfiguration::class.java)

    @Bean(name = ["taskExecutor"])
    override fun getAsyncExecutor(): AsyncTaskExecutor? {
        log.debug("Creating Async Task Executor")
        val executor = ThreadPoolTaskExecutor()
        executor.corePoolSize = 10
        executor.maxPoolSize = 20
        executor.queueCapacity = 30
        return executor
    }

    override fun getAsyncUncaughtExceptionHandler(): AsyncUncaughtExceptionHandler? {
        return SimpleAsyncUncaughtExceptionHandler()
    }

    /** Configure async support for Spring MVC.  */
    @Bean
    fun webMvcConfigurerConfigurer(
        taskExecutor: AsyncTaskExecutor?,
        callableProcessingInterceptor: CallableProcessingInterceptor?
    ): WebMvcConfigurer {
        return object : WebMvcConfigurer {
            override fun configureAsyncSupport(configurer: AsyncSupportConfigurer) {
                configurer.setDefaultTimeout(360000).setTaskExecutor(taskExecutor!!)
                configurer.registerCallableInterceptors(callableProcessingInterceptor)
                super.configureAsyncSupport(configurer)
            }
        }
    }

    @Bean
    fun callableProcessingInterceptor(): CallableProcessingInterceptor {
        return object : TimeoutCallableProcessingInterceptor() {
            @Throws(Exception::class)
            override fun <T> handleTimeout(request: NativeWebRequest, task: Callable<T>): Any {
                log.error("timeout!")
                return super.handleTimeout(request, task)
            }
        }
    }
}