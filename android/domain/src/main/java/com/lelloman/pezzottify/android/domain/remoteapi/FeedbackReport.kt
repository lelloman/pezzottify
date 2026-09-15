package com.lelloman.pezzottify.android.domain.remoteapi

import kotlinx.serialization.Serializable

@Serializable data class FeedbackAttachment(val kind:String,val content:String,val consent:Boolean=true)
@Serializable data class FeedbackReport(
    val clientRequestId:String,val kind:String,val category:String,val title:String?,val description:String,
    val clientType:String="android",val clientVersion:String?,val deviceInfo:String?,val attachments:List<FeedbackAttachment>,
)
@Serializable data class FeedbackReceipt(val id:String,val version:Long,val replayed:Boolean=false)
@Serializable data class FeedbackSummary(val id:String,val title:String?,val kind:String,val status:String)
@Serializable data class FeedbackPage(val items:List<FeedbackSummary>,val nextCursor:Long?=null)
sealed interface FeedbackResult {
    data class Sent(val id:String):FeedbackResult
    data class Failed(val status:Int?,val retryAfterSeconds:Long?=null):FeedbackResult
}
