package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.KSerializer
import kotlinx.serialization.PolymorphicSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.int
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put

internal class ActivityPeriodSerializer : KSerializer<ActivityPeriod> {

    override val descriptor: SerialDescriptor = PolymorphicSerializer(ActivityPeriod::class).descriptor

    override fun serialize(encoder: Encoder, value: ActivityPeriod) {
        require(encoder is JsonEncoder) { "This serializer can only be used with JSON" }
        val jsonObject = when (value) {
            is ActivityPeriod.Timespan -> buildJsonObject {
                put("Timespan", buildJsonObject {
                    put("start_year", value.startYear)
                    put("end_year", value.endYear)
                })
            }
            is ActivityPeriod.Decade -> buildJsonObject {
                put("Decade", value.value)
            }
        }
        encoder.encodeJsonElement(jsonObject)
    }

    override fun deserialize(decoder: Decoder): ActivityPeriod {
        require(decoder is JsonDecoder) { "This serializer can only be used with JSON" }
        val jsonElement = decoder.decodeJsonElement()
        require(jsonElement is JsonObject) { "Expected a JSON object" }
        return when (val key = jsonElement.keys.first()) {
            "Timespan" -> {
                val timespanObject = jsonElement[key] as JsonObject
                ActivityPeriod.Timespan(
                    startYear = timespanObject["start_year"]!!.jsonPrimitive.int,
                    endYear = timespanObject["end_year"]?.jsonPrimitive?.intOrNull
                )
            }
            "Decade" -> {
                ActivityPeriod.Decade(jsonElement[key]!!.jsonPrimitive.int)
            }
            else -> throw SerializationException("Unknown key: $key")
        }
    }
}

@Serializable(with = ActivityPeriodSerializer::class)
sealed interface ActivityPeriod {

    @Serializable
    data class Timespan(

        @SerialName("start_year")
        val startYear: Int,

        @SerialName("end_year")
        val endYear: Int?,
    ) : ActivityPeriod

    @Serializable
    data class Decade(
        val value: Int,
    ) : ActivityPeriod
}