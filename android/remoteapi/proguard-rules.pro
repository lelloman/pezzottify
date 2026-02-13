# Retrofit and Kotlin Serialization rules for remoteapi module

# Retrofit
-dontwarn retrofit2.**
-keep class retrofit2.** { *; }
-keep interface retrofit2.** { *; }
-keepattributes Signature
-keepattributes Exceptions

# Keep Retrofit service interfaces
-keepclasseswithmembers class * {
    @retrofit2.http.* <methods>;
}

# OkHttp
-dontwarn okhttp3.**
-dontwarn okio.**

# Kotlin Serialization
-keepattributes InnerClasses
-keepattributes *Annotation*
-keep @kotlinx.serialization.Serializable class * {*;}

# Keep response models
-keep class com.lelloman.pezzottify.android.remoteapi.model.** { *; }
