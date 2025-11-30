package com.lelloman.pezzottify.android.localdata.internal.statics

import androidx.room.TypeConverter
import com.lelloman.pezzottify.android.localdata.internal.statics.model.Disc
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

internal object StaticsDbTypesConverter {

    @TypeConverter
    fun fromListOfStringsToString(values: List<String>): String {
        return Json.encodeToString(values)
    }

    @TypeConverter
    fun fromStringToListOfStrings(value: String): List<String> {
        return Json.decodeFromString(value)
    }

    @TypeConverter
    fun fromDiscsListToString(values: List<Disc>): String {
        return Json.encodeToString(values)
    }

    @TypeConverter
    fun fromStringToDiscsList(value: String): List<Disc> {
        return Json.decodeFromString(value)
    }
}