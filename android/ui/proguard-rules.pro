# UI module - Jetpack Compose

# Keep all UI components (Compose uses reflection)
-keep class com.lelloman.pezzottify.android.ui.** { *; }

# Keep @Composable functions
-keepclasseswithmembers class * {
    @androidx.compose.runtime.Composable *;
}

# ViewModels
-keep class * extends androidx.lifecycle.ViewModel { *; }
-keep class * extends androidx.lifecycle.AndroidViewModel { *; }

# Hilt ViewModels
-keepclassmembers class * extends androidx.lifecycle.ViewModel {
    @androidx.hilt.lifecycle.ViewModelInject <init>(...);
}
