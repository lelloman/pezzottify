package com.lelloman.pezzottify.android.localdata.statics.internal

import androidx.room.TypeConverter
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
}