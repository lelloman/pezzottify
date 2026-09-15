package com.lelloman.pezzottify.android.ui.screen.main.settings.bugreport

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.ui.R

@OptIn(ExperimentalLayoutApi::class)
@Composable internal fun FeedbackOptions(state:BugReportScreenState,actions:BugReportScreenActions) {
    FlowRow(horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        for((id,label) in listOf("bug" to R.string.feedback_bug,"feature" to R.string.feedback_feature)) {
            FilterChip(selected=state.kind==id,onClick={actions.onKindChanged(id)},label={Text(stringResource(label))},enabled=!state.isSubmitting)
        }
    }
    FlowRow(horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        for((id,label) in listOf("assistant" to R.string.feedback_assistant,"playback" to R.string.feedback_playback,"downloads" to R.string.feedback_downloads,"ui" to R.string.feedback_interface,"other" to R.string.feedback_other)) {
            FilterChip(selected=state.category==id,onClick={actions.onCategoryChanged(id)},label={Text(stringResource(label))},enabled=!state.isSubmitting)
        }
    }
    Text(stringResource(R.string.feedback_privacy),style=MaterialTheme.typography.bodySmall)
    FeedbackToggle(stringResource(R.string.feedback_record),state.recordingEnabled,!state.isSubmitting,actions::onRecordingChanged)
    FeedbackToggle(stringResource(R.string.feedback_attach_chat),state.includeAssistant,!state.isSubmitting,actions::onIncludeAssistantChanged)
    if(state.includeAssistant) FeedbackToggle(stringResource(R.string.feedback_all_chats),state.includeAllChats,!state.isSubmitting,actions::onIncludeAllChatsChanged)
}
@Composable private fun FeedbackToggle(label:String,checked:Boolean,enabled:Boolean,change:(Boolean)->Unit) {
    Row(Modifier.fillMaxWidth(),verticalAlignment=androidx.compose.ui.Alignment.CenterVertically) {
        Text(label,Modifier.weight(1f));Switch(checked=checked,onCheckedChange=change,enabled=enabled)
    }
}
@Composable internal fun FeedbackPreview(state:BugReportScreenState) {
    state.retryAfterSeconds?.let {Text(stringResource(R.string.feedback_wait_seconds,it),style=MaterialTheme.typography.bodySmall)}
    var content by remember(state.preview) {mutableStateOf<String?>(null)}
    state.preview?.let {preview->
        Text(stringResource(R.string.feedback_preview_ready),style=MaterialTheme.typography.bodySmall)
        for(attachment in preview.request.attachments) {
            TextButton(onClick={content=attachment.content}) {
                Text(stringResource(R.string.feedback_preview_attachment,
                    stringResource(if(attachment.kind=="assistant") R.string.feedback_assistant else R.string.bug_report_include_logs),attachment.content.toByteArray().size))
            }
        }
    }
    content?.let {snapshot->
        val chunks=remember(snapshot) {buildList {var start=0;while(start<snapshot.length) {var end=minOf(start+2048,snapshot.length);if(end<snapshot.length && Character.isHighSurrogate(snapshot[end-1])) end--;add(snapshot.substring(start,end));start=end}}}
        AlertDialog(onDismissRequest={content=null},title={Text(stringResource(R.string.feedback_preview))},
            text={LazyColumn(Modifier.heightIn(max=420.dp)) {items(chunks) {Text(it,style=MaterialTheme.typography.bodySmall)}}},
            confirmButton={TextButton(onClick={content=null}) {Text(stringResource(R.string.back))}})
    }
}
@Composable internal fun FeedbackReports(state:BugReportScreenState,actions:BugReportScreenActions) {
    Text(stringResource(R.string.feedback_my_reports),style=MaterialTheme.typography.titleMedium)
    if(state.reportsError) Text(stringResource(R.string.feedback_reports_failed),color=MaterialTheme.colorScheme.error)
    TextButton(onClick={actions.refreshReports()},enabled=!state.reportsLoading) {Text(stringResource(R.string.feedback_refresh))}
    for(report in state.reports) {
        val status=when(report.status) {"new"->R.string.feedback_status_new;"investigating"->R.string.feedback_status_investigating;"planned"->R.string.feedback_status_planned;"resolved"->R.string.feedback_status_resolved;else->R.string.feedback_status_closed}
        Text("${report.title.orEmpty()}\n${report.id}\n${stringResource(status)}",Modifier.padding(vertical=8.dp),style=MaterialTheme.typography.bodySmall)
    }
    if(state.nextCursor!=null) TextButton(onClick={actions.refreshReports(true)},enabled=!state.reportsLoading) {Text(stringResource(R.string.feedback_more))}
}
