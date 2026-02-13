# ExoPlayer/Media3 rules for player module

-dontwarn androidx.media3.common.**
-keep class androidx.media3.** { *; }
-keep interface androidx.media3.** { *; }

# Keep player implementations
-keep class com.lelloman.pezzottify.android.player.** { *; }

# Keep listeners for reflection
-keep class * implements androidx.media3.common.Player$Listener { *; }
-keep class * implements androidx.media3.session.MediaSession$Callback { *; }

# Keep enums
-keepclassmembers enum androidx.media3.** {
    **[] $VALUES;
    public *;
}
