package com.lelloman.pezzottify.android.ui.screen.main.content

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.AssistChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.lelloman.pezzottify.android.domain.statics.EntityContributor
import com.lelloman.pezzottify.android.domain.statics.EntityEnrichmentStatus
import com.lelloman.pezzottify.android.domain.statics.EntityTag
import java.time.LocalDate
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException
import java.time.format.FormatStyle
import java.util.Locale

@Composable
fun EnrichmentStatusIndicator(
    status: EntityEnrichmentStatus?,
    entityType: String,
    modifier: Modifier = Modifier,
) {
    val normalizedStatus = status?.status ?: return
    if (normalizedStatus !in setOf("queued", "running", "failed")) return

    var showDialog by remember { mutableStateOf(false) }
    val statusLabel = normalizedStatus.replaceFirstChar { it.titlecase(Locale.getDefault()) }

    AssistChip(
        onClick = { showDialog = true },
        label = { Text("Enrichment: $statusLabel") },
        modifier = modifier,
    )

    if (showDialog) {
        AlertDialog(
            onDismissRequest = { showDialog = false },
            confirmButton = {
                TextButton(onClick = { showDialog = false }) {
                    Text("OK")
                }
            },
            title = { Text("$entityType enrichment") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(enrichmentStatusMessage(normalizedStatus))
                    EnrichmentFactRow("Status", statusLabel)
                    status.stage?.let { EnrichmentFactRow("Stage", it) }
                    EnrichmentFactRow("Attempts", status.attempts.toString())
                    status.lastError?.let { EnrichmentFactRow("Last error", it) }
                }
            },
        )
    }
}

@Composable
fun EnrichmentInfoBlock(
    summary: String?,
    facts: List<Pair<String, String>> = emptyList(),
    badges: List<String> = emptyList(),
    tags: List<EntityTag> = emptyList(),
    contributors: List<EntityContributor> = emptyList(),
    modifier: Modifier = Modifier,
) {
    if (summary.isNullOrBlank() && facts.isEmpty() && badges.isEmpty() && tags.isEmpty() && contributors.isEmpty()) {
        return
    }

    Surface(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 8.dp),
        tonalElevation = 1.dp,
        shape = MaterialTheme.shapes.medium,
    ) {
        Column(
            modifier = Modifier.padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            summary?.takeIf { it.isNotBlank() }?.let {
                Text(
                    text = it,
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                    maxLines = 5,
                    overflow = TextOverflow.Ellipsis,
                )
            }

            facts.forEach { (label, value) ->
                EnrichmentFactRow(label, value)
            }

            if (badges.isNotEmpty()) {
                Text(
                    text = badges.joinToString(" / "),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                )
            }

            if (tags.isNotEmpty()) {
                EnrichmentFactRow("Tags", tags.take(12).joinToString(", ") { it.tag })
            }

            if (contributors.isNotEmpty()) {
                EnrichmentFactRow(
                    "Credits",
                    contributors.take(8).joinToString(", ") { "${titleCase(it.role)}: ${it.contributorName}" },
                )
            }
        }
    }
}

@Composable
private fun EnrichmentFactRow(label: String, value: String) {
    Row(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelMedium,
            fontWeight = FontWeight.Bold,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(92.dp),
        )
        Spacer(modifier = Modifier.width(12.dp))
        Text(
            text = value,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
    }
}

private fun enrichmentStatusMessage(status: String): String = when (status) {
    "queued" -> "Pezzottify is waiting to generate richer metadata for this item."
    "running" -> "Pezzottify is currently generating richer metadata for this item."
    "failed" -> "Pezzottify tried to generate richer metadata for this item but the job failed."
    else -> "Pezzottify can generate richer metadata for catalog items in the background."
}

fun titleCase(value: String?): String? {
    if (value.isNullOrBlank()) return null
    return value.replace('_', ' ')
        .split(' ')
        .joinToString(" ") { word ->
            word.replaceFirstChar { char -> char.titlecase(Locale.getDefault()) }
        }
}

fun formatEnrichmentDate(value: String?): String? {
    if (value.isNullOrBlank()) return null
    val text = value.trim()
    if (Regex("^\\d{4}$").matches(text)) return text
    return try {
        when {
            Regex("^\\d{4}-\\d{2}$").matches(text) -> {
                val date = LocalDate.parse("$text-01")
                DateTimeFormatter.ofPattern("MMMM yyyy", Locale.getDefault()).format(date)
            }
            Regex("^\\d{4}-\\d{2}-\\d{2}$").matches(text) -> {
                val date = LocalDate.parse(text)
                DateTimeFormatter.ofLocalizedDate(FormatStyle.MEDIUM).format(date)
            }
            else -> text
        }
    } catch (_: DateTimeParseException) {
        text
    }
}

fun formatLanguageLabel(value: String?): String? {
    if (value.isNullOrBlank()) return null
    val code = value.trim().lowercase(Locale.getDefault())
    if (code in setOf("zxx", "und", "none", "instrumental")) return "Instrumental"
    return Locale.forLanguageTag(code).displayLanguage.takeIf { it.isNotBlank() } ?: titleCase(value)
}

fun joinMetadataParts(vararg parts: String?): String? {
    return parts.filterNot { it.isNullOrBlank() }.joinToString(" / ").takeIf { it.isNotBlank() }
}

fun flagBadges(vararg flags: Pair<Boolean?, String>): List<String> {
    return flags.filter { it.first == true }.map { it.second }
}

@Composable
fun EnrichmentSpacer() {
    Spacer(modifier = Modifier.height(8.dp))
}
