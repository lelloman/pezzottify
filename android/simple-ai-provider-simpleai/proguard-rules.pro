# Simple AI Provider - SimpleAI

# Keep provider classes
-keep class com.lelloman.simpleaiprovider.simpleai.** { *; }
-keep class com.lelloman.simpleaiprovider.api.simpleai.** { *; }

# OkHttp (warnings only - library handles its own obfuscation)
-dontwarn okhttp3.**
-dontwarn okio.**

# Keep data models for serialization
-keep @kotlinx.serialization.Serializable class com.lelloman.simpleaiprovider.simpleai.** { *; }
