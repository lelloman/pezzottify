# Room database rules for localdata module

# Keep Room database classes
-keep class * extends androidx.room.RoomDatabase
-keep @androidx.room.Entity class *
-keep @androidx.room.Dao class *
-dontwarn androidx.room.paging.**

# Keep Room entities and DAOs
-keep class com.lelloman.pezzottify.android.localdata.entity.** { *; }
-keep class com.lelloman.pezzottify.android.localdata.database.** { *; }
-keep class com.lelloman.pezzottify.android.localdata.store.** { *; }

# EncryptedSharedPreferences
-dontwarn androidx.security.crypto.**
-keep class androidx.security.crypto.** { *; }
