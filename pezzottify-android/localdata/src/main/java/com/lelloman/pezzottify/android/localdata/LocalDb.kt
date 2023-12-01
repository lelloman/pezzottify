package com.lelloman.pezzottify.android.localdata

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverter
import androidx.room.TypeConverters
import com.google.gson.Gson
import com.google.gson.reflect.TypeToken
import com.lelloman.pezzottify.android.localdata.model.Album
import com.lelloman.pezzottify.android.localdata.model.ArtistRelation
import com.lelloman.pezzottify.android.localdata.model.AudioTrack
import com.lelloman.pezzottify.android.localdata.model.BandArtist
import com.lelloman.pezzottify.android.localdata.model.Image
import com.lelloman.pezzottify.android.localdata.model.IndividualArtist
import java.lang.reflect.Type


object Converters {
    private val gson = Gson()

    private val stringsListType: Type = object : TypeToken<ArrayList<String?>?>() {}.type
    private val artistsRelationsListType: Type =
        object : TypeToken<ArrayList<ArtistRelation>?>() {}.type

    @TypeConverter
    fun stringsListFromString(value: String?): List<String> = gson.fromJson(value, stringsListType)

    @TypeConverter
    fun stringsLitToString(list: List<String>) = gson.toJson(list)

    @TypeConverter
    fun artistsRelationsToString(list: List<ArtistRelation>) = gson.toJson(list)

    @TypeConverter
    fun artistsRelationsFromString(value: String?): List<ArtistRelation> =
        gson.fromJson(value, artistsRelationsListType)
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