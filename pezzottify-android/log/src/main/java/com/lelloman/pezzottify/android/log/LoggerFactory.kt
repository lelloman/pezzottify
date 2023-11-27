package com.lelloman.pezzottify.android.log

import kotlin.reflect.KProperty

interface LoggerFactory {

    operator fun getValue(obj: Any, property: KProperty<*>): Logger
}
