package app.noted.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshots.SnapshotStateMap
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

val BoldStyle = SpanStyle(fontWeight = FontWeight.Bold, color = C.White)
val ItalicStyle = SpanStyle(fontStyle = FontStyle.Italic)
val StrikeStyle = SpanStyle(textDecoration = TextDecoration.LineThrough, color = C.Muted)
val CodeStyle = SpanStyle(background = C.Guide, color = C.White, fontSize = 13.sp)
val LinkStyle = SpanStyle(textDecoration = TextDecoration.Underline, color = C.White)

/** Markdown inline -> styled text, with the syntax characters removed. */
fun renderInline(src: String, links: Boolean = false): AnnotatedString {
    val spans = mutableListOf<InlineSpan>()
    scanInline(src, 0, src.length, spans)
    if (spans.isEmpty()) return AnnotatedString(src)
    val hidden = BooleanArray(src.length)
    for (s in spans) if (s.kind == InlineKind.MARK || s.kind == InlineKind.URL) for (k in s.start until s.end) hidden[k] = true
    val map = IntArray(src.length + 1)
    val sb = StringBuilder()
    for (k in src.indices) {
        map[k] = sb.length
        if (!hidden[k]) sb.append(src[k])
    }
    map[src.length] = sb.length
    return buildAnnotatedString {
        append(sb.toString())
        for (s in spans) {
            val a = map[s.start]
            val b = map[s.end]
            if (a >= b) continue
            when (s.kind) {
                InlineKind.BOLD -> addStyle(BoldStyle, a, b)
                InlineKind.ITALIC -> addStyle(ItalicStyle, a, b)
                InlineKind.STRIKE -> addStyle(StrikeStyle, a, b)
                InlineKind.CODE -> addStyle(CodeStyle, a, b)
                InlineKind.LINK -> if (links && s.url != null) {
                    addLink(LinkAnnotation.Url(s.url, TextLinkStyles(LinkStyle)), a, b)
                } else addStyle(LinkStyle, a, b)
                else -> {}
            }
        }
    }
}

/** Rendered markdown with collapsible list items and tappable checkboxes. */
@Composable
fun MarkdownReader(src: String, onToggleTask: (Int) -> Unit, modifier: Modifier = Modifier) {
    val blocks = remember(src) { parseBlocks(src) }
    val collapsed = remember { mutableStateMapOf<Int, Boolean>() }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        blocks.forEachIndexed { idx, b ->
            when (b) {
                is Block.Heading -> Text(
                    renderInline(b.text, true),
                    fontSize = when (b.level) { 1 -> 24.sp; 2 -> 18.sp; 3 -> 16.sp; else -> 14.sp },
                    lineHeight = when (b.level) { 1 -> 30.sp; 2 -> 24.sp; else -> 21.sp },
                    fontWeight = FontWeight.Bold, letterSpacing = if (b.level == 1) (-0.4).sp else 0.sp,
                    color = if (b.level >= 4) C.Text2 else C.White,
                    modifier = if (b.level > 1 && idx > 0) Modifier.padding(top = 8.dp) else Modifier,
                )
                is Block.Paragraph -> Text(renderInline(b.text, true), fontSize = 14.sp, lineHeight = 22.sp, color = C.Text2)
                is Block.Quote -> Row(Modifier.height(IntrinsicSize.Min)) {
                    Box(Modifier.width(2.dp).fillMaxHeight().background(C.White))
                    Text(
                        renderInline(b.text, true), fontSize = 14.sp, lineHeight = 22.sp, color = C.Text3,
                        fontStyle = FontStyle.Italic, modifier = Modifier.padding(start = 14.dp, top = 2.dp, bottom = 2.dp),
                    )
                }
                is Block.Code -> Box(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(C.CodeBg)
                        .border(1.dp, C.Guide, RoundedCornerShape(14.dp)).horizontalScroll(rememberScrollState()).padding(14.dp),
                ) { Text(b.text, fontSize = 13.sp, lineHeight = 20.sp, color = C.Text2, softWrap = false) }
                Block.Rule -> Box(Modifier.fillMaxWidth().padding(vertical = 4.dp).height(1.dp).background(C.Border))
                is Block.ListBlock -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    val g = b.items.any { it.children.isNotEmpty() }
                    b.items.forEach { ListItem(it, g, collapsed, onToggleTask) }
                }
            }
        }
    }
}

@Composable
private fun ListItem(n: ListNode, gutter: Boolean, collapsed: SnapshotStateMap<Int, Boolean>, onToggleTask: (Int) -> Unit) {
    val hasKids = n.children.isNotEmpty()
    val isClosed = collapsed[n.line] == true
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.Top, modifier = if (gutter) Modifier.offset(x = (-10).dp) else Modifier) {
            // Chevron gutter (only items with children can fold).
            if (gutter) Box(
                Modifier.size(28.dp).clip(RoundedCornerShape(14.dp))
                    .then(if (hasKids) Modifier.clickable(onClickLabel = if (isClosed) "Expand" else "Collapse") { collapsed[n.line] = !isClosed } else Modifier),
                contentAlignment = Alignment.Center,
            ) {
                if (hasKids) Icon(
                    Icons.Chevron, null, tint = C.Muted,
                    modifier = Modifier.size(14.dp).rotate(if (isClosed) 0f else 90f),
                )
            }
            Box(Modifier.height(28.dp), contentAlignment = Alignment.Center) {
                when (n.info.kind) {
                    LineKind.TASK -> Box(
                        Modifier.padding(end = 10.dp).size(16.dp).clip(RoundedCornerShape(4.dp))
                            .then(if (n.info.checked) Modifier.background(C.White) else Modifier.border(1.5.dp, C.Muted, RoundedCornerShape(4.dp)))
                            .clickable(onClickLabel = if (n.info.checked) "Uncheck" else "Check") { onToggleTask(n.line) },
                        contentAlignment = Alignment.Center,
                    ) {
                        if (n.info.checked) Icon(Icons.Checkmark, null, tint = C.Black, modifier = Modifier.size(12.dp))
                    }
                    LineKind.NUMBER -> Text(n.info.number, fontSize = 14.sp, color = C.Muted, modifier = Modifier.padding(end = 8.dp))
                    else -> Box(Modifier.padding(end = 10.dp).size(5.dp).clip(RoundedCornerShape(3.dp)).background(C.White))
                }
            }
            Column(Modifier.padding(top = 3.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        renderInline(n.text, true), fontSize = 14.sp, lineHeight = 22.sp,
                        color = if (n.info.checked) C.Muted else C.White,
                        textDecoration = if (n.info.checked) TextDecoration.LineThrough else null,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (hasKids && isClosed) {
                        val count = countDescendants(n)
                        Text(
                            if (count == 1) "1 item" else "$count items", fontSize = 11.sp, color = C.Muted,
                            modifier = Modifier.padding(start = 10.dp).border(1.dp, C.Border, RoundedCornerShape(10.dp)).padding(horizontal = 8.dp),
                        )
                    }
                }
            }
        }
        if (hasKids && !isClosed) {
            Row(Modifier.height(IntrinsicSize.Min).padding(start = if (gutter) 3.dp else 2.dp)) {
                Box(Modifier.width(1.dp).fillMaxHeight().background(C.Guide))
                Column(Modifier.padding(start = 18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    val g = n.children.any { it.children.isNotEmpty() }
                    n.children.forEach { ListItem(it, g, collapsed, onToggleTask) }
                }
            }
        }
    }
}

private fun countDescendants(n: ListNode): Int = n.children.sumOf { 1 + countDescendants(it) }
