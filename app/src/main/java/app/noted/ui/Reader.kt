package app.noted.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.runtime.getValue
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
val CodeStyle = SpanStyle(background = C.Guide, color = C.White, fontSize = 13.sp, fontFamily = Mono)
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

private sealed class Node {
    data class Leaf(val block: Block) : Node()
    data class Section(val heading: Block.Heading, val children: List<Node>) : Node()
}

/** Nest blocks under their headings: a section runs until the next heading of the same or higher level. */
private fun buildTree(blocks: List<Block>): List<Node> {
    var i = 0
    fun parse(minLevel: Int): List<Node> {
        val out = mutableListOf<Node>()
        while (i < blocks.size) {
            val b = blocks[i]
            if (b is Block.Heading) {
                if (b.level <= minLevel) return out
                i++
                out += Node.Section(b, parse(b.level))
            } else {
                out += Node.Leaf(b); i++
            }
        }
        return out
    }
    return parse(0)
}

/** Rendered markdown with foldable heading sections, collapsible list items and tappable checkboxes. */
@Composable
fun MarkdownReader(
    src: String,
    folded: Set<String>,
    onToggleFold: (String) -> Unit,
    onToggleTask: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val tree = remember(src) { buildTree(parseBlocks(src)) }
    val collapsed = remember { mutableStateMapOf<Int, Boolean>() }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(14.dp)) {
        tree.forEachIndexed { idx, n -> RenderNode(n, idx == 0, folded, onToggleFold, collapsed, onToggleTask) }
    }
}

@Composable
private fun RenderNode(
    n: Node,
    first: Boolean,
    folded: Set<String>,
    onToggleFold: (String) -> Unit,
    collapsed: SnapshotStateMap<Int, Boolean>,
    onToggleTask: (Int) -> Unit,
) {
    when (n) {
        is Node.Leaf -> RenderBlock(n.block, collapsed, onToggleTask)
        is Node.Section -> {
            val b = n.heading
            val foldable = n.children.isNotEmpty()
            val closed = foldable && b.raw in folded
            Column {
                Box(Modifier.padding(top = if (b.level > 1 && !first) 8.dp else 0.dp)) {
                    if (foldable) FoldChevron(closed, Modifier.offset(x = (-30).dp, y = headingChevronY(b.level))) { onToggleFold(b.raw) }
                    Text(
                        renderInline(b.text, true),
                        fontSize = headingSize(b.level),
                        lineHeight = when (b.level) { 1 -> 28.sp; 2 -> 25.sp; else -> 23.sp },
                        fontWeight = if (b.level <= 2) FontWeight.ExtraBold else FontWeight.Bold,
                        letterSpacing = if (b.level == 1) (-0.4).sp else 0.sp,
                        color = if (b.level >= 4) C.Text2 else C.White,
                        modifier = if (foldable) Modifier.clickable(
                            interactionSource = null, indication = null,
                            onClickLabel = if (closed) "Expand section" else "Collapse section",
                        ) { onToggleFold(b.raw) } else Modifier,
                    )
                }
                AnimatedVisibility(
                    visible = !closed,
                    enter = expandVertically(tween(220)) + fadeIn(tween(220)),
                    exit = shrinkVertically(tween(200)) + fadeOut(tween(140)),
                ) {
                    Column(Modifier.padding(top = 14.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                        n.children.forEach { RenderNode(it, false, folded, onToggleFold, collapsed, onToggleTask) }
                    }
                }
            }
        }
    }
}

/** Centre the 28dp chevron on the heading's first line. */
private fun headingChevronY(level: Int) = when (level) { 1 -> 0.dp; 2 -> (-1).dp; else -> (-2).dp }

@Composable
private fun RenderBlock(b: Block, collapsed: SnapshotStateMap<Int, Boolean>, onToggleTask: (Int) -> Unit) {
    when (b) {
        is Block.Heading -> {}
        is Block.Paragraph -> Text(renderInline(b.text, true), fontSize = 15.sp, lineHeight = 24.sp, color = C.Text2)
        is Block.Quote -> Row(Modifier.height(IntrinsicSize.Min)) {
            Box(Modifier.width(3.dp).fillMaxHeight().clip(RoundedCornerShape(2.dp)).background(C.White))
            Text(
                renderInline(b.text, true), fontSize = 15.sp, lineHeight = 24.sp, color = C.Text3,
                fontStyle = FontStyle.Italic, modifier = Modifier.padding(start = 14.dp, top = 2.dp, bottom = 2.dp),
            )
        }
        is Block.Code -> Box(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(C.CodeBg)
                .border(LineW, C.Guide, RoundedCornerShape(14.dp)).horizontalScroll(rememberScrollState()).padding(14.dp),
        ) { Text(b.text, fontFamily = Mono, fontSize = 13.sp, lineHeight = 20.sp, color = C.Text2, softWrap = false) }
        Block.Rule -> Box(Modifier.fillMaxWidth().padding(vertical = 4.dp).height(2.dp).background(C.Border))
        is Block.ListBlock -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            val g = b.items.any { it.children.isNotEmpty() }
            b.items.forEach { ListItem(it, g, collapsed, onToggleTask) }
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
                val angle by animateFloatAsState(if (isClosed) 0f else 90f, spring(stiffness = 700f), label = "listChevron")
                if (hasKids) Icon(
                    Icons.Chevron, null, tint = if (isClosed) C.White else C.Muted,
                    modifier = Modifier.size(14.dp).rotate(angle),
                )
            }
            Box(Modifier.height(28.dp), contentAlignment = Alignment.Center) {
                when (n.info.kind) {
                    LineKind.TASK -> Box(
                        Modifier.padding(end = 10.dp).size(16.dp).clip(RoundedCornerShape(4.dp))
                            .then(if (n.info.checked) Modifier.background(C.White) else Modifier.border(2.dp, C.Muted, RoundedCornerShape(4.dp)))
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
                        renderInline(n.text, true), fontSize = 15.sp, lineHeight = 22.sp,
                        color = if (n.info.checked) C.Muted else C.White,
                        textDecoration = if (n.info.checked) TextDecoration.LineThrough else null,
                        modifier = Modifier.weight(1f, fill = false),
                    )
                    if (hasKids && isClosed) {
                        val count = countDescendants(n)
                        Text(
                            if (count == 1) "1 item" else "$count items", fontSize = 11.sp, color = C.Muted,
                            modifier = Modifier.padding(start = 10.dp).border(LineW, C.Border, RoundedCornerShape(10.dp)).padding(horizontal = 8.dp),
                        )
                    }
                }
            }
        }
        AnimatedVisibility(
            visible = hasKids && !isClosed,
            enter = expandVertically(tween(200)) + fadeIn(tween(200)),
            exit = shrinkVertically(tween(180)) + fadeOut(tween(120)),
        ) {
            Row(Modifier.height(IntrinsicSize.Min).padding(start = if (gutter) 3.dp else 2.dp)) {
                Box(Modifier.width(2.dp).fillMaxHeight().background(C.Guide))
                Column(Modifier.padding(start = 18.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    val g = n.children.any { it.children.isNotEmpty() }
                    n.children.forEach { ListItem(it, g, collapsed, onToggleTask) }
                }
            }
        }
    }
}

private fun countDescendants(n: ListNode): Int = n.children.sumOf { 1 + countDescendants(it) }
