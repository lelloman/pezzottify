package com.lelloman.debuginterface

import com.google.gson.GsonBuilder
import com.lelloman.debuginterface.internal.DebugServer

class DebugServerBuilder {

    private val operations = mutableListOf<DebugOperation>()
    private var started = false

    private var port = 8889
    private var gsonBuilder = GsonBuilder()

    fun add(operation: DebugOperation) = apply {
        operations.add(operation)
    }

    fun setPort(port: Int) = apply {
        this.port = port
    }

    fun setGson(gsonBuilder: GsonBuilder) = apply {
        this.gsonBuilder = gsonBuilder
    }

    fun start() = synchronized(this) {
        if (started) throw IllegalStateException("DebugServer already started!")
        started = true
        DebugServer(
            operations = operations,
            port = port,
            gsonBuilder = gsonBuilder,
        ).start()
    }
}
