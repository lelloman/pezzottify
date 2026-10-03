# Extra keep rules for Paravoid's payload R8 (paravoid.minifyPayload), on top of
# proguard-rules.pro. From Paravoid's compatibility/minification/runtime-validation.md:
# classes reached by name at runtime that the normal release rules do not cover.
-keep @dagger.hilt.android.lifecycle.HiltViewModel class * extends androidx.lifecycle.ViewModel { *; }
-keep class * extends androidx.lifecycle.ViewModel { <init>(...); }
-keepattributes RuntimeVisibleAnnotations
-keep class * extends androidx.navigation.Navigator { *; }
-keep class * extends androidx.startup.Initializer { <init>(); }
# ui ships androidx.compose.ui:ui-tooling-android, whose animation inspection code
# references Android Studio-only classes. They are never loaded at runtime.
-dontwarn androidx.compose.animation.tooling.**
