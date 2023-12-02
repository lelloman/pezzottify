package com.lelloman.pezzottify.android.app.localdata

import android.content.Context
import com.google.gson.GsonBuilder
import com.lelloman.pezzottify.android.app.di.IoDispatcher
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.withContext
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton
import kotlin.reflect.KClass

class PersistentObjectDef<T : Any>(val key: String, val type: KClass<T>) {
    init {
        require(key.isNotBlank())
        require(key.all { it.isLetterOrDigit() })
        require(key.length in 4..32)
    }
}

@Singleton
class ObjectsStore @Inject constructor(
    @ApplicationContext private val context: Context,
    defs: Set<@JvmSuppressWildcards PersistentObjectDef<*>>,
    gsonBuilder: GsonBuilder,
    @IoDispatcher private val ioDispatcher: CoroutineDispatcher,
) {

    private val gson = gsonBuilder.create()

    private val defsMap = HashMap<String, KClass<*>>()

    init {
        defs.forEach { def ->
            if (defsMap.containsKey(def.key)) {
                throw IllegalStateException("Found duplicate key \"${def.key}\"")
            }
            defsMap[def.key] = def.type
        }
    }

    private val PersistentObjectDef<*>.file get() = File(context.filesDir, key)

    suspend fun <T : Any> store(def: PersistentObjectDef<T>, obj: T) = withContext(ioDispatcher) {
        assertKeyExists(def.key)

        val jsonString = gson.toJson(obj)
        def.file.writeText(jsonString)
    }

    suspend fun <T : Any> load(def: PersistentObjectDef<T>): T = withContext(ioDispatcher) {
        assertKeyExists(def.key)
        val json = File(context.filesDir, def.key).readText()
        gson.fromJson<T>(json, defsMap[def.key]!!.java)
    }

    suspend fun <T : Any> delete(def: PersistentObjectDef<T>) = withContext(ioDispatcher) {
        assertKeyExists(def.key)
        defsMap.remove(def.key)
        def.file.delete()
    }

    private fun assertKeyExists(key: String) {
        if (defsMap.containsKey(key).not()) {
            throw IllegalArgumentException("Unknown key \"${key}\"")
        }
    }
}