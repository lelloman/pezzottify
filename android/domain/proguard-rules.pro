# Domain module - keep domain models and use cases

# Keep domain models for serialization
-keep @kotlinx.serialization.Serializable class com.lelloman.pezzottify.android.domain.** { *; }

# Keep use case interfaces and implementations
-keep interface com.lelloman.pezzottify.android.domain.** { *; }
-keep class * implements com.lelloman.pezzottify.android.domain.** { *; }

# Kotlin coroutines
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}
