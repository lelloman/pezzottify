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

# java.util.ServiceLoader providers listed in the payload's META-INF/services. AGP's R8
# keeps these automatically; Paravoid's payload R8 does not, and R8 then removes the
# providers (nothing references them in code) or renames the service interfaces (the
# file name must match). Shipping the shrunk payload without these crashed at startup:
# "Provider kotlinx.coroutines.android.AndroidDispatcherFactory not found".
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory
-keep class kotlinx.coroutines.android.AndroidDispatcherFactory { <init>(); }
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler
-keep class kotlinx.coroutines.android.AndroidExceptionPreHandler { <init>(); }
-keepnames class coil3.util.FetcherServiceLoaderTarget
-keep class coil3.network.okhttp.internal.OkHttpNetworkFetcherServiceLoaderTarget { <init>(); }
-keepnames class com.fasterxml.jackson.core.JsonFactory
-keepnames class com.fasterxml.jackson.core.ObjectCodec
-keep class com.fasterxml.jackson.core.JsonFactory { <init>(); }
-keep class com.fasterxml.jackson.dataformat.yaml.YAMLFactory { <init>(); }
-keep class com.fasterxml.jackson.dataformat.yaml.YAMLMapper { <init>(); }
-keep class com.fasterxml.jackson.databind.ObjectMapper { <init>(); }
