# Simple AI Provider - Ollama

# Keep provider classes
-keep class com.lelloman.simpleaiprovider.ollama.** { *; }
-keep class com.lelloman.simpleaiprovider.api.ollama.** { *; }

# OkHttp (warnings only - library handles its own obfuscation)
-dontwarn okhttp3.**
-dontwarn okio.**

# Keep data models for serialization
-keep @kotlinx.serialization.Serializable class com.lelloman.simpleaiprovider.ollama.** { *; }
