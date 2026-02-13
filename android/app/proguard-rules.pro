# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.

# ============================================
# General Rules
# ============================================

# Keep line number information for debugging stack traces
-keepattributes SourceFile,LineNumberTable

# If you keep line number information, rename the source file
-renamesourcefileattribute SourceFile

# Preserve native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# ============================================
# Hilt/Dagger
# ============================================

-keep class dagger.hilt.** { *; }
-keep class javax.inject.** { *; }
-keep class dagger.** { *; }

# Keep Hilt generated classes
-keep public class * extends dagger.hilt.android.internal.managers.ViewComponentManager$FragmentContextWrapper
-keep public class * extends dagger.hilt.android.internal.managers.ViewComponentManager$ActivityContextWrapper

# Keep Hilt AndroidEntryPoints
-keep @dagger.hilt.android.AndroidEntryPoint class * { *; }
-keep @dagger.hilt.android.HiltAndroidApp class * { *; }

# Keep generic signatures for reflection
-keepattributes Signature
-keepattributes *Annotation*
-keepattributes InnerClasses
-keepattributes EnclosingMethod

# Keep Hilt module classes
-keepclassmembers class * {
    @dagger.* <fields>;
    @dagger.* <methods>;
}

# ============================================
# Room
# ============================================

-keep class * extends androidx.room.RoomDatabase
-keep @androidx.room.Entity class *
-keep @androidx.room.Dao class *
-dontwarn androidx.room.paging.**

# Keep Room column info
-keepclassmembers class * {
    @androidx.room.ColumnInfo <fields>;
}

# ============================================
# Retrofit
# ============================================

-dontwarn retrofit2.**
-keep class retrofit2.** { *; }
-keep interface retrofit2.** { *; }
-keepattributes Signature
-keepattributes Exceptions

# Keep Retrofit service method parameters
-keepclasseswithmembers class * {
    @retrofit2.http.* <methods>;
}

# Keep Retrofit response classes
-keepclassmembers class * {
    @com.squareup.moshi.Json <fields>;
    @com.google.gson.annotations.SerializedName <fields>;
}

# ============================================
# OkHttp
# ============================================

-dontwarn okhttp3.**
-dontwarn okio.**
-keep class okhttp3.** { *; }
-keep interface okhttp3.** { *; }

# ============================================
# Kotlin Serialization
# ============================================

-keepclassmembers class kotlinx.serialization.** {
    *** ...;
}
-keep @kotlinx.serialization.Serializable class * {*;}
-keepclassmembers class * {
    *** Companion;
}

# ============================================
# Kotlin Coroutines
# ============================================

-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}
-keepclassmembernames class kotlinx.coroutines.** {
    volatile <fields>;
}

# ============================================
# Jetpack Compose
# ============================================

-keep class androidx.compose.** { *; }
-keep class kotlin.Metadata { *; }

# Keep @Composable functions
-keepclasseswithmembers class * {
    @androidx.compose.runtime.Composable *;
}

# ============================================
# ExoPlayer/Media3
# ============================================

-dontwarn androidx.media3.common.**
-keep class androidx.media3.** { *; }
-keep interface androidx.media3.** { *; }

# Keep listeners for reflection
-keep class * implements androidx.media3.common.Player$Listener { *; }
-keep class * implements androidx.media3.session.MediaSession$Callback { *; }

# Keep enums
-keepclassmembers enum androidx.media3.** {
    **[] $VALUES;
    public *;
}

# ============================================
# Coil
# ============================================

-keep class coil.** { *; }
-dontwarn coil.**

# ============================================
# AppAuth (OIDC)
# ============================================

-dontwarn net.openid.appauth.**
-keep class net.openid.appauth.** { *; }

# ============================================
# WorkManager
# ============================================

-dontwarn androidx.work.**
-keep class androidx.work.** { *; }

# ============================================
# Lifecycle / ViewModel
# ============================================

-keep class * extends androidx.lifecycle.ViewModel { *; }
-keep class * extends androidx.lifecycle.AndroidViewModel { *; }
-keep class * extends androidx.lifecycle.LiveData { *; }

# ============================================
# Navigation
# ============================================

-keep public class * extends androidx.navigation.NavDestination
-keep public class * implements androidx.navigation.NavigatorProvider

# ============================================
# DuckMapper (code generation)
# ============================================

-keep @com.github.lelloman.duckmapper.annotations.Mapper class * { *; }
-keepclassmembers class * {
    @com.github.lelloman.duckmapper.annotations.MapperField <fields>;
}

# ============================================
# Simple AI Assistant
# ============================================

-keep class com.lelloman.simpleaiassistant.** { *; }
-keep class com.lelloman.simpleaiprovider.** { *; }

# Keep data classes for serialization in AI modules
-keep @kotlinx.serialization.Serializable class com.lelloman.simpleaiassistant.** { *; }
-keep @kotlinx.serialization.Serializable class com.lelloman.simpleaiprovider.** { *; }

# ============================================
# Project-specific classes
# ============================================

# Keep domain models (they may use reflection)
-keep class com.lelloman.pezzottify.android.domain.** { *; }
-keep interface com.lelloman.pezzottify.android.domain.** { *; }

# Keep remote API models (serialization)
-keep class com.lelloman.pezzottify.android.remoteapi.model.** { *; }

# Keep localdata entities
-keep class com.lelloman.pezzottify.android.localdata.entity.** { *; }
-keep class com.lelloman.pezzottify.android.localdata.database.** { *; }

# Keep player implementations
-keep class com.lelloman.pezzottify.android.player.** { *; }

# Keep UI components (Compose)
-keep class com.lelloman.pezzottify.android.ui.** { *; }

# Keep logger
-keep class com.lelloman.pezzottify.android.logger.** { *; }

# Keep BuildConfig
-keep class com.lelloman.pezzottify.android.BuildConfig { *; }

# ============================================
# Security / EncryptedSharedPreferences
# ============================================

-dontwarn androidx.security.crypto.**
-keep class androidx.security.crypto.** { *; }

# ============================================
# JSON serialization
# ============================================

-keepclassmembers class * {
    @kotlinx.serialization.SerialName <fields>;
}

# ============================================
# KSP-generated classes
# ============================================

-keep class **_Impl { *; }
-keep class **_Factory { *; }
-keep class **$HiltModule { *; }
-keepclassmembers class * {
    @androidx.hilt.* <methods>;
}

# ============================================
# Instrumentation tests (only for test builds)
# ============================================

-dontwarn androidx.test.**
-dontwarn junit.**
-dontwarn org.junit.**
-dontwarn mockk.**
-dontwarn io.mockk.**

# ============================================
# Google Tink (transitive dependency from security-crypto)
# ============================================

-dontwarn com.google.api.client.**
-dontwarn com.google.crypto.tink.util.KeysDownloader
-dontwarn org.joda.time.**
