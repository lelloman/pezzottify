package com.lelloman.simpleaiassistant.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp

/**
 * A composable that renders markdown-formatted text.
 *
 * Supports:
 * - **bold** or __bold__
 * - *italic* or _italic_
 * - `inline code`
 * - ```code blocks```
 * - - bullet lists
 * - * bullet lists
 * - [links](url)
 */
@Composable
fun MarkdownText(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = MaterialTheme.typography.bodyMedium,
    color: Color = MaterialTheme.colorScheme.onSurface
) {
    val codeBackgroundColor = MaterialTheme.colorScheme.surfaceContainerHighest
    val codeTextColor = MaterialTheme.colorScheme.onSurfaceVariant
    val linkColor = MaterialTheme.colorScheme.primary

    val blocks = remember(text) { parseMarkdownBlocks(text) }

    Column(modifier = modifier) {
        blocks.forEach { block ->
            when (block) {
                is MarkdownBlock.CodeBlock -> {
                    Surface(
                        color = codeBackgroundColor,
                        shape = RoundedCornerShape(6.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 4.dp)
                    ) {
                        Text(
                            text = block.content,
                            style = style.copy(
                                fontFamily = FontFamily.Monospace,
                                color = codeTextColor
                            ),
                            modifier = Modifier.padding(8.dp)
                        )
                    }
                }
                is MarkdownBlock.Paragraph -> {
                    val annotatedString = remember(block.content, color, linkColor, codeBackgroundColor, codeTextColor) {
                        parseInlineMarkdown(
                            text = block.content,
                            baseColor = color,
                            linkColor = linkColor,
                            codeBackgroundColor = codeBackgroundColor,
                            codeTextColor = codeTextColor
                        )
                    }

                    // Use BasicText which handles LinkAnnotation automatically
                    BasicText(
                        text = annotatedString,
                        style = style.copy(color = color)
                    )
                }
            }
        }
    }
}

private sealed class MarkdownBlock {
    data class CodeBlock(val content: String, val language: String?) : MarkdownBlock()
    data class Paragraph(val content: String) : MarkdownBlock()
}

/**
 * Parses markdown text into blocks (code blocks vs regular paragraphs).
 */
private fun parseMarkdownBlocks(text: String): List<MarkdownBlock> {
    val blocks = mutableListOf<MarkdownBlock>()
    val codeBlockPattern = Regex("```(\\w*)?\\n?([\\s\\S]*?)```")

    var lastEnd = 0
    codeBlockPattern.findAll(text).forEach { match ->
        // Add any text before this code block
        if (match.range.first > lastEnd) {
            val before = text.substring(lastEnd, match.range.first).trim()
            if (before.isNotEmpty()) {
                blocks.add(MarkdownBlock.Paragraph(before))
            }
        }

        // Add the code block
        val language = match.groupValues[1].takeIf { it.isNotEmpty() }
        val code = match.groupValues[2].trim()
        if (code.isNotEmpty()) {
            blocks.add(MarkdownBlock.CodeBlock(code, language))
        }

        lastEnd = match.range.last + 1
    }

    // Add any remaining text after the last code block
    if (lastEnd < text.length) {
        val remaining = text.substring(lastEnd).trim()
        if (remaining.isNotEmpty()) {
            blocks.add(MarkdownBlock.Paragraph(remaining))
        }
    }

    // If no code blocks found, treat entire text as paragraph
    if (blocks.isEmpty() && text.isNotBlank()) {
        blocks.add(MarkdownBlock.Paragraph(text))
    }

    return blocks
}

/**
 * Parses inline markdown elements (bold, italic, code, links, lists).
 */
private fun parseInlineMarkdown(
    text: String,
    baseColor: Color,
    linkColor: Color,
    codeBackgroundColor: Color,
    codeTextColor: Color
): AnnotatedString {
    return buildAnnotatedString {
        var i = 0
        val lines = text.split('\n')

        lines.forEachIndexed { lineIndex, line ->
            // Check for list items
            val listMatch = Regex("^\\s*[-*]\\s+").find(line)
            if (listMatch != null) {
                append("  \u2022 ")
                parseInlineLine(
                    line.substring(listMatch.range.last + 1),
                    baseColor,
                    linkColor,
                    codeBackgroundColor,
                    codeTextColor,
                    this
                )
            } else {
                parseInlineLine(line, baseColor, linkColor, codeBackgroundColor, codeTextColor, this)
            }

            if (lineIndex < lines.size - 1) {
                append('\n')
            }
        }
    }
}

private fun parseInlineLine(
    line: String,
    baseColor: Color,
    linkColor: Color,
    codeBackgroundColor: Color,
    codeTextColor: Color,
    builder: AnnotatedString.Builder
) {
    var i = 0

    while (i < line.length) {
        when {
            // Bold with ** or __
            line.startsWith("**", i) || line.startsWith("__", i) -> {
                val delimiter = line.substring(i, i + 2)
                val endIndex = line.indexOf(delimiter, i + 2)
                if (endIndex != -1) {
                    builder.withStyle(SpanStyle(fontWeight = FontWeight.Bold)) {
                        append(line.substring(i + 2, endIndex))
                    }
                    i = endIndex + 2
                } else {
                    builder.append(line[i])
                    i++
                }
            }

            // Italic with * or _ (but not ** or __)
            (line[i] == '*' || line[i] == '_') && (i + 1 >= line.length || line[i + 1] != line[i]) -> {
                val delimiter = line[i]
                val endIndex = line.indexOf(delimiter, i + 1)
                if (endIndex != -1 && endIndex > i + 1) {
                    builder.withStyle(SpanStyle(fontStyle = FontStyle.Italic)) {
                        append(line.substring(i + 1, endIndex))
                    }
                    i = endIndex + 1
                } else {
                    builder.append(line[i])
                    i++
                }
            }

            // Inline code with `
            line[i] == '`' -> {
                val endIndex = line.indexOf('`', i + 1)
                if (endIndex != -1) {
                    builder.withStyle(
                        SpanStyle(
                            fontFamily = FontFamily.Monospace,
                            background = codeBackgroundColor,
                            color = codeTextColor
                        )
                    ) {
                        append(" ${line.substring(i + 1, endIndex)} ")
                    }
                    i = endIndex + 1
                } else {
                    builder.append(line[i])
                    i++
                }
            }

            // Links [text](url)
            line[i] == '[' -> {
                val closeBracket = line.indexOf(']', i + 1)
                if (closeBracket != -1 && closeBracket + 1 < line.length && line[closeBracket + 1] == '(') {
                    val closeParen = line.indexOf(')', closeBracket + 2)
                    if (closeParen != -1) {
                        val linkText = line.substring(i + 1, closeBracket)
                        val url = line.substring(closeBracket + 2, closeParen)

                        builder.withLink(
                            LinkAnnotation.Url(
                                url = url,
                                styles = TextLinkStyles(
                                    style = SpanStyle(
                                        color = linkColor,
                                        textDecoration = TextDecoration.Underline
                                    )
                                )
                            )
                        ) {
                            append(linkText)
                        }
                        i = closeParen + 1
                    } else {
                        builder.append(line[i])
                        i++
                    }
                } else {
                    builder.append(line[i])
                    i++
                }
            }

            else -> {
                builder.append(line[i])
                i++
            }
        }
    }
}
