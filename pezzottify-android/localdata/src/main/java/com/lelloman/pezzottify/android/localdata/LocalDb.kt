package com.lelloman.pezzottify.android.localdata

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import com.lelloman.pezzottify.android.localdata.model.Album
import com.lelloman.pezzottify.android.localdata.model.AudioTrack
import com.lelloman.pezzottify.android.localdata.model.BandArtist
import com.lelloman.pezzottify.android.localdata.model.Image
import com.lelloman.pezzottify.android.localdata.model.IndividualArtist
import java.lang.reflect.Type


object Converters {
    private val gson = Gson()
    private val listType: Type = object : TypeToken<ArrayList<String?>?>() {}.type

    @TypeConverter
    fun fromString(value: String?): List<String> = gson.fromJson(value, listType)

    @TypeConverter
    fun fromList(list: List<String>) = gson.toJson(list)
}

@Database(
    entities = [
        IndividualArtist::class,
        BandArtist::class,
        Image::class,
        Album::class,
        AudioTrack::class
    ],
    version = 1
)
@TypeConverters(Converters::class)
abstract class LocalDb : RoomDatabase() {

    abstract fun staticsDao(): StaticsDao

    companion object {
        const val DB_NAME = "LocalDb"
    }
}