# Simple AI Assistant - generic module

# Keep the AI assistant classes
-keep class com.lelloman.simpleaiassistant.** { *; }

# Keep data classes for serialization
-keep @kotlinx.serialization.Serializable class com.lelloman.simpleaiassistant.** { *; }

# Keep KSP generated classes
-keep class **_Impl { *; }
-keep class **_Factory { *; }

# Keep Compose UI
-keepclasseswithmembers class * {
    @androidx.compose.runtime.Composable *;
}
