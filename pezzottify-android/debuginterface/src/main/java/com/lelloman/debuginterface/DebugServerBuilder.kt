package com.lelloman.debuginterface

import com.lelloman.debuginterface.internal.DebugServer

class DebugServerBuilder {

    private val operations = mutableListOf<DebugOperation>()

    fun add(operation: DebugOperation) = apply {
        operations.add(operation)
    }

    fun start() {
        DebugServer(operations).start()
    }
}
