package app.noted.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.style.TextIndent
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.TextUnit
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.ParagraphStyle
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.IntOffset
import androidx.compose.foundation.layout.offset
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import kotlin.math.roundToInt
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/* ------------------------------------------------------------------ */
/* Live preview: raw markdown only on the line(s) holding the cursor. */
/* ------------------------------------------------------------------ */

private val Hidden = SpanStyle(color = Color.Transparent, fontSize = 0.01.sp, letterSpacing = 0.sp)
private val Invisible = SpanStyle(color = Color.Transparent)
private val Dim = SpanStyle(color = C.Syntax, fontWeight = FontWeight.Normal, fontStyle = FontStyle.Normal, textDecoration = TextDecoration.None)

/** Body text size in the reader and the editor. */
val BodySize = 16.sp
val CodeSize = 14.sp

/** Heading sizes are capped so every glyph fits inside the shared 28sp line box. */
fun headingSize(level: Int) = when (level) { 1 -> 22.sp; 2 -> 19.sp; 3 -> 17.sp; else -> BodySize }

/** Extra width given to each leading space or tab, so nested lists and indented lines step in further. */
private val IndentWide = SpanStyle(letterSpacing = 6.sp)

/** One line box height for every line. Glyphs stay inside their own box, so the active-line pill never touches its neighbours. */
private val BodyLine = 28.sp
private val CenteredLines = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.None)
private val Folded = ParagraphStyle(lineHeight = 0.01.sp, lineHeightStyle = CenteredLines)

/** Collapsible heading sections: heading line index -> first line index after its section. */
fun sectionEnds(lines: List<String>, infos: List<LineInfo>): Map<Int, Int> {
    val out = HashMap<Int, Int>()
    for (i in lines.indices) {
        val info = infos[i]
        if (info.kind != LineKind.HEADING) continue
        var j = i + 1
        while (j < lines.size && !(infos[j].kind == LineKind.HEADING && infos[j].level <= info.level)) j++
        // Only headings with something under them can fold.
        if ((i + 1 until j).any { infos[it].kind != LineKind.BLANK }) out[i] = j
    }
    return out
}

/** Key that identifies a heading across edits and between reading and editing mode. */
fun headingKey(line: String) = line.trim()

/** Lines hidden because a heading above them is folded (unless the cursor is inside). */
fun foldedLines(lines: List<String>, infos: List<LineInfo>, folded: Set<String>): BooleanArray {
    val hidden = BooleanArray(lines.size)
    if (folded.isEmpty()) return hidden
    for ((h, end) in sectionEnds(lines, infos)) {
        if (hidden[h] || headingKey(lines[h]) !in folded) continue
        for (k in h + 1 until end) hidden[k] = true
    }
    return hidden
}

/** Character range [start, end) of the raw lines touched by the selection. */
fun activeRange(text: String, sel: TextRange): IntRange {
    val a = minOf(sel.start, sel.end).coerceIn(0, text.length)
    val b = maxOf(sel.start, sel.end).coerceIn(0, text.length)
    val s = text.lastIndexOf('\n', a - 1) + 1
    val e = text.indexOf('\n', b).let { if (it < 0) text.length else it }
    return s..e
}

class LivePreview(private val active: IntRange, private val folded: Set<String>, private val hangs: Map<Int, TextUnit> = emptyMap()) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val chars = raw.toCharArray()
        val base = mutableListOf<AnnotatedString.Range<SpanStyle>>()
        val top = mutableListOf<AnnotatedString.Range<SpanStyle>>()
        val paras = mutableListOf<AnnotatedString.Range<ParagraphStyle>>()
        val lines = raw.split('\n')
        val infos = classifyLines(lines)
        val hiddenLines = foldedLines(lines, infos, folded)
        var s = 0
        for ((i, l) in lines.withIndex()) {
            val e = s + l.length
            val pEnd = if (e < raw.length) e + 1 else e
            if (hiddenLines[i]) {
                // Folded under a heading: squash the whole line to nothing.
                if (pEnd > s) {
                    paras += AnnotatedString.Range(Folded, s, pEnd)
                    top += AnnotatedString.Range(Hidden, s, pEnd)
                }
                s = e + 1
                continue
            }
            val isActive = s <= active.last && e >= active.first
            // Wrapped rows line up with the text after the marker or indent.
            hangs[i]?.let { h -> if (pEnd > s) paras += AnnotatedString.Range(ParagraphStyle(textIndent = TextIndent(restLine = h)), s, pEnd) }
            fun hide(a: Int, b: Int) { if (b > a) top += AnnotatedString.Range(if (isActive) Dim else Hidden, a, b) }
            fun ghost(a: Int, b: Int) { if (b > a) top += AnnotatedString.Range(if (isActive) Dim else Invisible, a, b) }
            fun style(st: SpanStyle, a: Int, b: Int) { if (b > a) base += AnnotatedString.Range(st, a, b) }
            fun inline(from: Int) {
                val spans = mutableListOf<InlineSpan>()
                scanInline(raw, from, e, spans)
                for (sp in spans) when (sp.kind) {
                    InlineKind.MARK, InlineKind.URL -> hide(sp.start, sp.end)
                    InlineKind.BOLD -> style(BoldStyle, sp.start, sp.end)
                    InlineKind.ITALIC -> style(ItalicStyle, sp.start, sp.end)
                    InlineKind.STRIKE -> style(StrikeStyle, sp.start, sp.end)
                    InlineKind.CODE -> style(CodeStyle, sp.start, sp.end)
                    InlineKind.LINK -> style(LinkStyle, sp.start, sp.end)
                }
            }
            val info = infos[i]
            // Widen the leading indentation (not inside code blocks, where alignment matters).
            if (info.kind != LineKind.CODE && info.kind != LineKind.FENCE && info.kind != LineKind.BLANK) {
                var w = 0
                while (w < l.length && (l[w] == ' ' || l[w] == '\t')) w++
                if (w > 0) base += AnnotatedString.Range(IndentWide, s, s + w)
            }
            when (info.kind) {
                LineKind.HEADING -> {
                    style(SpanStyle(fontSize = headingSize(info.level), fontWeight = FontWeight.Bold, color = C.White), s, e)
                    hide(s, s + info.prefixEnd)
                    inline(s + info.prefixEnd)
                }
                LineKind.TASK -> {
                    val open = l.indexOf('[', info.markerStart)
                    hide(s + info.markerStart, s + open)
                    ghost(s + open, s + open + 3)
                    if (isActive) style(Dim, s + open + 3, s + info.prefixEnd)
                    if (info.checked) style(SpanStyle(color = C.Muted, textDecoration = TextDecoration.LineThrough), s + info.prefixEnd, e)
                    inline(s + info.prefixEnd)
                }
                LineKind.BULLET -> {
                    if (isActive) style(Dim, s + info.markerStart, s + info.prefixEnd)
                    else chars[s + info.markerStart] = '•'
                    inline(s + info.prefixEnd)
                }
                LineKind.NUMBER -> {
                    style(if (isActive) Dim else SpanStyle(color = C.Muted), s + info.markerStart, s + info.prefixEnd)
                    inline(s + info.prefixEnd)
                }
                LineKind.QUOTE -> {
                    ghost(s + info.markerStart, s + info.prefixEnd)
                    style(SpanStyle(fontStyle = FontStyle.Italic, color = C.Text3), s + info.prefixEnd, e)
                    inline(s + info.prefixEnd)
                }
                LineKind.RULE -> ghost(s, e)
                LineKind.FENCE -> style(Dim.copy(fontFamily = Mono), s, e)
                LineKind.CODE -> style(SpanStyle(color = C.Text2, fontFamily = Mono, fontSize = CodeSize), s, e)
                LineKind.TEXT -> {
                    style(SpanStyle(color = C.Text2), s, e)
                    inline(s)
                }
                LineKind.BLANK -> {}
            }
            s = e + 1
        }
        return TransformedText(AnnotatedString(String(chars), base + top, paras), OffsetMapping.Identity)
    }

    override fun equals(other: Any?) = other is LivePreview && other.active == active && other.folded == folded && other.hangs == hangs
    override fun hashCode() = (active.hashCode() * 31 + folded.hashCode()) * 31 + hangs.hashCode()
}

private sealed class Deco {
    data class Box(val open: Int, val checked: Boolean) : Deco()
    data class Bar(val start: Int, val end: Int) : Deco()
    data class Rule(val start: Int) : Deco()
    data class Code(val start: Int, val end: Int) : Deco()
}

private fun decorations(raw: String, active: IntRange, folded: Set<String>): List<Deco> {
    val out = mutableListOf<Deco>()
    val lines = raw.split('\n')
    val infos = classifyLines(lines)
    val hidden = foldedLines(lines, infos, folded)
    var s = 0
    var fenceStart = -1
    for ((i, l) in lines.withIndex()) {
        val e = s + l.length
        val isActive = s <= active.last && e >= active.first
        val info = infos[i]
        if (hidden[i]) {
            if (fenceStart >= 0) { out += Deco.Code(fenceStart, maxOf(fenceStart, s - 1)); fenceStart = -1 }
            s = e + 1
            continue
        }
        when (info.kind) {
            LineKind.FENCE -> if (fenceStart < 0) fenceStart = s else { out += Deco.Code(fenceStart, e); fenceStart = -1 }
            LineKind.TASK -> if (!isActive) out += Deco.Box(s + l.indexOf('[', info.markerStart), info.checked)
            LineKind.QUOTE -> if (!isActive) out += Deco.Bar(s + info.markerStart, e)
            LineKind.RULE -> if (!isActive) out += Deco.Rule(s)
            else -> {}
        }
        s = e + 1
    }
    if (fenceStart >= 0) out += Deco.Code(fenceStart, raw.length)
    return out
}

/** What the block menu (the ⋮ handle) can do. */
enum class BlockOp { CUT, COPY, DUPLICATE, UP, DOWN, DELETE }

/** Apply a block-menu action to the block holding line [line]. Returns the new value, or null. */
fun applyBlockOp(v: TextFieldValue, line: Int, op: BlockOp, folded: Set<String>, copy: (String) -> Unit): TextFieldValue? {
    val lines = v.text.split('\n')
    if (line !in lines.indices) return null
    val infos = classifyLines(lines)
    val blk = blockAt(lines, line, infos)
    val piece = lines.subList(blk.first, blk.last).joinToString("\n")
    fun startOf(i: Int) = lines.take(i).sumOf { it.length + 1 }
    return when (op) {
        BlockOp.COPY -> { copy(piece + "\n"); null }
        BlockOp.CUT, BlockOp.DELETE -> {
            if (op == BlockOp.CUT) copy(piece + "\n")
            val rest = lines.subList(0, blk.first) + lines.subList(blk.last, lines.size)
            val t = rest.joinToString("\n")
            val at = rest.take(blk.first).sumOf { it.length + 1 }.coerceAtMost(t.length)
            TextFieldValue(t, TextRange(at))
        }
        BlockOp.DUPLICATE -> {
            val out = lines.subList(0, blk.last) + lines.subList(blk.first, blk.last) + lines.subList(blk.last, lines.size)
            val t = out.joinToString("\n")
            val at = (startOf(blk.last) + lines[blk.first].length).coerceAtMost(t.length)
            TextFieldValue(t, TextRange(at))
        }
        BlockOp.UP, BlockOp.DOWN -> {
            val hidden = foldedLines(lines, infos, folded)
            val m = nudgeBlock(lines, blk, hidden, op == BlockOp.DOWN) ?: return null
            TextFieldValue(m.text, TextRange(remapOffset(lines, m.order, v.selection.start), remapOffset(lines, m.order, v.selection.end)))
        }
    }
}

/** A block being dragged by its handle. Offsets are in px. */
private class BlockDrag(val block: LineSpan, val units: List<LineSpan>, val home: Int, val tops: FloatArray, val bottoms: FloatArray, val serial: Long) {
    var fingerStart = 0f
    var finger by mutableFloatStateOf(0f)
    var scrollStart = 0
    val top get() = tops[block.first]
    val height get() = bottoms[block.last - 1] - tops[block.first]

    /** The unit index the block would be dropped before, for a block offset of [dy]. */
    fun gap(dy: Float): Int {
        val centre = top + dy + height / 2f
        var g = 0
        for ((j, u) in units.withIndex()) {
            val ut = tops[u.first]
            val ub = bottoms[u.last - 1]
            val ct = if (ut >= bottoms[block.last - 1]) ut - height else ut
            if (ct + (ub - ut) / 2f < centre) g = j + 1
        }
        return g
    }

    fun target(j: Int, gap: Int): Float {
        val ut = tops[units[j].first]
        val ct = if (ut >= bottoms[block.last - 1]) ut - height else ut
        return ct + (if (j >= gap) height else 0f) - ut
    }
}

/** The active-line pill: a near-black card with a fine light outline. */
val PillFill = Color(0xFF111111)
val PillLine = Color(0xFF2A2A2A)
private val HandleBg = Color(0xFF1F1F1F)
/** Selected text sits on a soft white highlight with rounded corners. */
val SelectionFill = Color.White.copy(alpha = 0.24f)
/** Room kept at the right of the text for the block handle. */
private val HandleRoom = 18.dp

@Composable
fun LiveEditor(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    focus: FocusRequester,
    folded: Set<String>,
    onToggleFold: (String) -> Unit,
    modifier: Modifier = Modifier,
    onLayout: (TextLayoutResult) -> Unit = {},
    scroll: ScrollState? = null,
    viewport: () -> Rect? = { null },
) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    // While text is selected the preview stays put on the line the selection started from, so
    // dragging a selection handle doesn't reflow the text (and the magnifier) under the finger.
    val anchor = if (value.selection.collapsed) value.selection else TextRange(value.selection.start)
    val active = activeRange(value.text, anchor)
    val lines = remember(value.text) { value.text.split('\n') }
    val infos = remember(lines) { classifyLines(lines) }
    val sections = remember(lines, infos) { sectionEnds(lines, infos) }
    val lineStarts = remember(lines) {
        val out = IntArray(lines.size)
        var p = 0
        for (i in lines.indices) { out[i] = p; p += lines[i].length + 1 }
        out
    }
    // A fold whose section holds the cursor opens up, so you never type into hidden text.
    val effectiveFolded = remember(folded, sections, active, lines) {
        if (folded.isEmpty()) folded
        else folded.filterTo(HashSet()) { key ->
            sections.none { (h, end) ->
                headingKey(lines[h]) == key && active.last >= lineStarts[h] + lines[h].length + 1 &&
                    active.first < (if (end < lines.size) lineStarts[end] else value.text.length + 1)
            }
        }
    }
    val density = LocalDensity.current
    // Wrapped list items, quotes and indented lines: how far in their text starts (sp), by line.
    var hangs by remember { mutableStateOf<Map<Int, TextUnit>>(emptyMap()) }
    val transformation = remember(active, effectiveFolded, hangs) { LivePreview(active, effectiveFolded, hangs) }
    val decos = remember(value.text, active, effectiveFolded) { decorations(value.text, active, effectiveFolded) }
    val textStyle = remember {
        TextStyle(fontFamily = Sans, fontSize = BodySize, lineHeight = BodyLine, lineHeightStyle = CenteredLines, color = C.White)
    }
    val hiddenLines = remember(lines, infos, effectiveFolded) { foldedLines(lines, infos, effectiveFolded) }
    val clipboard = LocalClipboardManager.current
    val latest by rememberUpdatedState(value)
    val latestFolded by rememberUpdatedState(folded)

    // Where the active-line pill should sit; animated so it glides between lines.
    val lay = layout
    var pillTarget = Rect.Zero
    val half = with(density) { BodyLine.toPx() / 2f - 2.dp.toPx() }
    if (lay != null) {
        val len = lay.layoutInput.text.length
        if (active.first <= len) {
            val raw = value.text
            var first = active.first
            while (first < active.last && first < raw.length && (raw[first] == ' ' || raw[first] == '\t')) first++
            val x = if (first < len) lay.getHorizontalPosition(first, true) else 0f
            // Centre on the letters themselves (midway between baseline and cap height), not on the
            // line box: the font's ascent/descent padding is lopsided, which made the pill look off-centre.
            fun mid(off: Int): Float {
                val o = off.coerceIn(0, len)
                val rawLine = (lineStarts.indexOfLast { it <= o }).coerceAtLeast(0)
                val info = infos.getOrNull(rawLine)
                val size = if (info?.kind == LineKind.HEADING) headingSize(info.level) else BodySize
                val fontPx = with(density) { size.toPx() }
                return lay.getLineBaseline(lay.getLineForOffset(o)) - fontPx * 0.355f
            }
            pillTarget = Rect(x, mid(active.first) - half, 0f, mid(active.last) + half)
        }
    }
    val spec = spring<Float>(stiffness = 900f)
    val pillTop by animateFloatAsState(pillTarget.top, spec, label = "pillTop")
    val pillBottom by animateFloatAsState(pillTarget.bottom, spec, label = "pillBottom")
    val pillX by animateFloatAsState(pillTarget.left, spec, label = "pillX")

    // Dragging a block: every other unit slides to make room.
    var drag by remember { mutableStateOf<BlockDrag?>(null) }
    val d = drag
    val dragDy = if (d != null) d.finger - d.fingerStart + ((scroll?.value ?: 0) - d.scrollStart) else 0f
    val unitOffsets = HashMap<Int, Float>()
    if (d != null) {
        val gap = d.gap(dragDy)
        d.units.forEachIndexed { j, u ->
            key(d.serial, u.first) {
                val o by animateFloatAsState(d.target(j, gap), spring(stiffness = 600f, dampingRatio = 0.85f), label = "slide")
                unitOffsets[u.first] = o
            }
        }
    }
    val lineOff = remember(d, lines) { FloatArray(lines.size) }
    if (d != null) {
        lineOff.fill(0f)
        for (u in d.units) { val o = unitOffsets[u.first] ?: 0f; for (i in u.first until u.last) if (i < lineOff.size) lineOff[i] = o }
        for (i in d.block.first until d.block.last) if (i < lineOff.size) lineOff[i] = dragDy
    }
    fun lineIndexOf(off: Int): Int = (lineStarts.indexOfLast { it <= off }).coerceAtLeast(0)
    fun dyAt(off: Int): Float = if (d == null) 0f else lineOff.getOrElse(lineIndexOf(off)) { 0f }

    // Keep the view scrolling while a block is held near the top or bottom edge.
    var fingerWin by remember { mutableFloatStateOf(0f) }
    LaunchedEffect(d) {
        if (d == null || scroll == null) return@LaunchedEffect
        val edge = with(density) { 64.dp.toPx() }
        while (true) {
            withFrameNanos { }
            val vp = viewport() ?: continue
            val speed = when {
                fingerWin < vp.top + edge -> -14f
                fingerWin > vp.bottom - edge -> 14f
                else -> 0f
            }
            if (speed != 0f) scroll.scrollBy(speed)
        }
    }

    fun startDrag() {
        val l = layout ?: return
        val v = latest
        val ls = v.text.split('\n')
        val inf = classifyLines(ls)
        val len = l.layoutInput.text.length
        val starts = IntArray(ls.size).also { var p = 0; for (i in ls.indices) { it[i] = p; p += ls[i].length + 1 } }
        val tops = FloatArray(ls.size) { l.getLineTop(l.getLineForOffset(starts[it].coerceIn(0, len))) }
        val bottoms = FloatArray(ls.size) { l.getLineBottom(l.getLineForOffset((starts[it] + ls[it].length).coerceIn(0, len))) }
        val cur = (starts.indexOfLast { it <= v.selection.start.coerceIn(0, v.text.length) }).coerceAtLeast(0)
        val blk = blockAt(ls, cur, inf)
        val hidden = foldedLines(ls, inf, latestFolded)
        val us = dragUnits(ls, blk, hidden, inf)
        drag = BlockDrag(blk, us, homeGap(blk, us), tops, bottoms, System.nanoTime()).also {
            it.fingerStart = fingerWin
            it.finger = fingerWin
            it.scrollStart = scroll?.value ?: 0
        }
    }
    fun endDrag(commit: Boolean) {
        val dd = drag ?: return
        drag = null
        if (!commit) return
        val dy = dd.finger - dd.fingerStart + ((scroll?.value ?: 0) - dd.scrollStart)
        val gap = dd.gap(dy)
        if (gap == dd.home) return
        val v = latest
        val ls = v.text.split('\n')
        val m = moveBlock(ls, dd.block, dd.units, gap)
        onValueChange(TextFieldValue(m.text, TextRange(remapOffset(ls, m.order, v.selection.start), remapOffset(ls, m.order, v.selection.end))))
    }

    Box(
        modifier.drawBehind {
            val lay2 = layout ?: return@drawBehind
            val len = lay2.layoutInput.text.length
            fun lineTop(off: Int) = lay2.getLineTop(lay2.getLineForOffset(off.coerceIn(0, len))) + dyAt(off)
            fun lineBottom(off: Int) = lay2.getLineBottom(lay2.getLineForOffset(off.coerceIn(0, len))) + dyAt(off)
            val right = size.width + 12.dp.toPx()
            // Code blocks.
            for (dc in decos) if (dc is Deco.Code) {
                if (d != null && lineIndexOf(dc.start) in d.block) continue
                drawCodeBox(lineTop(dc.start), lineBottom(dc.end), size.width)
            }
            // The line being edited: a near-black card with a fine outline, the handle at its right end.
            if (d == null && active.first <= len && pillBottom > pillTop) {
                val tl = Offset(pillX - 12.dp.toPx(), pillTop)
                val sz = Size(right - tl.x, pillBottom - pillTop)
                drawRoundRect(PillFill, tl, sz, CornerRadius(12.dp.toPx()))
                drawRoundRect(PillLine, Offset(tl.x + 0.5.dp.toPx(), tl.y + 0.5.dp.toPx()), Size(sz.width - 1.dp.toPx(), sz.height - 1.dp.toPx()), CornerRadius(12.dp.toPx()), style = Stroke(1.dp.toPx()))
            }
            // Selected text: rounded highlights, one per row.
            val sel = value.selection
            if (!sel.collapsed && d == null) {
                val a = sel.min.coerceIn(0, len)
                val b = sel.max.coerceIn(0, len)
                val la = lay2.getLineForOffset(a)
                val lb = lay2.getLineForOffset(b)
                val pad = 3.dp.toPx()
                for (ln in la..lb) {
                    val ls = lay2.getLineStart(ln)
                    val le = lay2.getLineEnd(ln, visibleEnd = true)
                    val s = maxOf(a, ls)
                    val e = minOf(b, le)
                    val x0 = lay2.getHorizontalPosition(s, true)
                    val x1 = if (b > le) lay2.getLineRight(ln) + 4.dp.toPx() else lay2.getHorizontalPosition(e, true)
                    if (x1 - x0 < 1f) continue
                    drawRoundRect(
                        SelectionFill, Offset(x0 - 1.dp.toPx(), lay2.getLineTop(ln) + pad),
                        Size(x1 - x0 + 2.dp.toPx(), lay2.getLineBottom(ln) - lay2.getLineTop(ln) - 2 * pad), CornerRadius(6.dp.toPx()),
                    )
                }
            }
            for (dc in decos) {
                if (d != null && dc !is Deco.Code) {
                    val at = when (dc) { is Deco.Box -> dc.open; is Deco.Bar -> dc.start; is Deco.Rule -> dc.start; is Deco.Code -> dc.start }
                    if (lineIndexOf(at) in d.block) continue
                }
                drawDeco(dc, lay2, ::lineTop, ::lineBottom, ::dyAt)
            }
        },
    ) {
        CompositionLocalProvider(LocalTextSelectionColors provides NotedSelectionColors) {
            BasicTextField(
                value = value,
                onValueChange = { if (drag == null) onValueChange(it) },
                textStyle = textStyle,
                visualTransformation = transformation,
                onTextLayout = { l ->
                    layout = l
                    onLayout(l)
                    val nh = computeHangs(l, lines, infos, lineStarts, hiddenLines, density)
                    if (nh != hangs) hangs = nh
                },
                cursorBrush = SolidColor(C.White),
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, autoCorrectEnabled = true),
                modifier = Modifier.fillMaxWidth().padding(end = HandleRoom).focusRequester(focus)
                    .drawWithContent {
                        val dd = drag
                        val l = layout
                        if (dd == null || l == null) { drawContent(); return@drawWithContent }
                        // While dragging, paint the text in pieces, each at its own offset.
                        val n = lines.size
                        var i = 0
                        while (i < n) {
                            if (i in dd.block) { i = dd.block.last; continue }
                            val o = lineOff[i]
                            var j = i + 1
                            while (j < n && j !in dd.block && kotlin.math.abs(lineOff[j] - o) < 0.25f) j++
                            val top = dd.tops[i]
                            val bottom = dd.bottoms[j - 1]
                            translate(top = o) { clipRect(left = -2000f, top = top, right = size.width + 2000f, bottom = bottom) { drawText(l) } }
                            i = j
                        }
                        // The dragged block floats above the rest on its own card with a soft shadow.
                        val bt = dd.top + dragDy
                        val bb = bt + dd.height
                        val x0 = -12.dp.toPx()
                        val x1 = size.width + HandleRoom.toPx() + 12.dp.toPx()
                        for (k in 1..6) {
                            val g = k * 2.5f.dp.toPx()
                            drawRoundRect(Color.Black.copy(alpha = 0.10f), Offset(x0 - g / 2, bt - g / 3 + 8.dp.toPx()), Size(x1 - x0 + g, bb - bt + g), CornerRadius(12.dp.toPx() + g))
                        }
                        drawRoundRect(Color(0xFF171717), Offset(x0, bt), Size(x1 - x0, bb - bt), CornerRadius(12.dp.toPx()))
                        drawRoundRect(Color(0xFF383838), Offset(x0, bt), Size(x1 - x0, bb - bt), CornerRadius(12.dp.toPx()), style = Stroke(1.dp.toPx()))
                        val len = l.layoutInput.text.length
                        for (dc in decos) {
                            val at = when (dc) { is Deco.Box -> dc.open; is Deco.Bar -> dc.start; is Deco.Rule -> dc.start; is Deco.Code -> dc.start }
                            if (lineIndexOf(at) !in dd.block) continue
                            fun lt(off: Int) = l.getLineTop(l.getLineForOffset(off.coerceIn(0, len))) + dragDy
                            fun lb(off: Int) = l.getLineBottom(l.getLineForOffset(off.coerceIn(0, len))) + dragDy
                            if (dc is Deco.Code) drawCodeBox(lt(dc.start), lb(dc.end), size.width + HandleRoom.toPx()) else drawDeco(dc, l, ::lt, ::lb) { dragDy }
                        }
                        translate(top = dragDy) {
                            clipRect(left = -2000f, top = dd.top, right = size.width + 2000f, bottom = dd.top + dd.height) { drawText(l) }
                        }
                        // Its handle rides along.
                        val hc = Offset(size.width + HandleRoom.toPx() - 6.dp.toPx(), bt + half)
                        drawHandle(hc, 1f)
                    },
            )
        }
        // Fold chevrons in the left gutter, one per heading that has content.
        val l = layout
        if (l != null) {
            val len = l.layoutInput.text.length
            for ((h, _) in sections) {
                if (hiddenLines[h]) continue
                val start = lineStarts[h].coerceIn(0, len)
                val ln = l.getLineForOffset(start)
                val mid = l.getLineBaseline(ln) - with(density) { headingSize(infos[h].level).toPx() } * 0.355f + dyAt(start)
                val key = headingKey(lines[h])
                val closed = key in effectiveFolded
                FoldChevron(
                    closed = closed,
                    modifier = Modifier.offset {
                        IntOffset(-with(density) { 30.dp.roundToPx() }, (mid - with(density) { 14.dp.toPx() }).roundToInt())
                    },
                ) { onToggleFold(key) }
            }
        }
        // The block handle at the right end of the pill: drag it to move the block, tap for actions.
        if (l != null && active.first <= l.layoutInput.text.length) {
            var menu by remember { mutableStateOf(false) }
            var coords by remember { mutableStateOf<androidx.compose.ui.layout.LayoutCoordinates?>(null) }
            val cur = remember(value.selection, lines) { lineIndexOf(value.selection.start.coerceIn(0, value.text.length)) }
            Box(
                Modifier.align(Alignment.TopEnd)
                    .offset { IntOffset(with(density) { 8.dp.roundToPx() }, (pillTop + half - with(density) { 18.dp.toPx() }).roundToInt()) }
                    .size(36.dp)
                    .onGloballyPositioned { coords = it }
                    .graphicsLayer { alpha = if (drag == null) 1f else 0f }
                    .pointerInput(Unit) { detectTapGestures(onTap = { menu = true }) }
                    .pointerInput(Unit) {
                        detectDragGestures(
                            onDragStart = { p ->
                                fingerWin = coords?.localToWindow(p)?.y ?: 0f
                                startDrag()
                            },
                            onDrag = { ch, _ ->
                                ch.consume()
                                val y = coords?.localToWindow(ch.position)?.y ?: return@detectDragGestures
                                fingerWin = y
                                drag?.finger = y
                            },
                            onDragEnd = { endDrag(true) },
                            onDragCancel = { endDrag(false) },
                        )
                    }
                    .semantics { contentDescription = "Block actions" },
                contentAlignment = Alignment.Center,
            ) {
                Box(Modifier.size(26.dp).clip(RoundedCornerShape(9.dp)).background(HandleBg), contentAlignment = Alignment.Center) {
                    Icon(Icons.More, null, tint = C.Text3, modifier = Modifier.size(15.dp))
                }
                Menu(
                    menu, { menu = false },
                    listOf(
                        "Cut" to { applyBlockOp(latest, cur, BlockOp.CUT, latestFolded) { clipboard.setText(AnnotatedString(it)) }?.let(onValueChange); Unit },
                        "Copy" to { applyBlockOp(latest, cur, BlockOp.COPY, latestFolded) { clipboard.setText(AnnotatedString(it)) }; Unit },
                        "Duplicate" to { applyBlockOp(latest, cur, BlockOp.DUPLICATE, latestFolded) {}?.let(onValueChange); Unit },
                        "Move up" to { applyBlockOp(latest, cur, BlockOp.UP, latestFolded) {}?.let(onValueChange); Unit },
                        "Move down" to { applyBlockOp(latest, cur, BlockOp.DOWN, latestFolded) {}?.let(onValueChange); Unit },
                        "Delete" to { applyBlockOp(latest, cur, BlockOp.DELETE, latestFolded) {}?.let(onValueChange); Unit },
                    ),
                )
            }
        }
    }
}

/** Handles and the caret stay white; the highlight itself is drawn by the editor, rounded. */
val NotedSelectionColors = TextSelectionColors(handleColor = C.White, backgroundColor = Color.Transparent)

private fun DrawScope.drawHandle(c: Offset, a: Float) {
    val s = 26.dp.toPx()
    drawRoundRect(HandleBg.copy(alpha = a), Offset(c.x - s / 2, c.y - s / 2), Size(s, s), CornerRadius(9.dp.toPx()))
    for (k in -1..1) drawCircle(C.Text3.copy(alpha = a), 1.6.dp.toPx(), Offset(c.x, c.y + k * 5.dp.toPx()))
}

private fun DrawScope.drawCodeBox(top: Float, bottom: Float, width: Float) {
    drawRoundRect(C.CodeBg, topLeft = Offset(-10.dp.toPx(), top), size = Size(width + 20.dp.toPx(), bottom - top), cornerRadius = CornerRadius(14.dp.toPx()))
    drawRoundRect(C.Guide, topLeft = Offset(-10.dp.toPx(), top), size = Size(width + 20.dp.toPx(), bottom - top), cornerRadius = CornerRadius(14.dp.toPx()), style = Stroke(LineW.toPx()))
}

private fun DrawScope.drawDeco(d: Deco, lay2: TextLayoutResult, lineTop: (Int) -> Float, lineBottom: (Int) -> Float, dyAt: (Int) -> Float) {
    val len = lay2.layoutInput.text.length
    when (d) {
        is Deco.Box -> if (d.open + 2 < len) {
            val a = lay2.getBoundingBox(d.open)
            val b = lay2.getBoundingBox(d.open + 2)
            val side = 16.dp.toPx()
            val cx = (a.left + b.right) / 2f
            val cy = lay2.getLineBaseline(lay2.getLineForOffset(d.open.coerceIn(0, len))) - BodySize.toPx() * 0.355f + dyAt(d.open)
            val tl = Offset(cx - side / 2, cy - side / 2)
            if (d.checked) {
                drawRoundRect(C.White, tl, Size(side, side), CornerRadius(4.dp.toPx()))
                val p = androidx.compose.ui.graphics.Path().apply {
                    moveTo(tl.x + side * 0.25f, tl.y + side * 0.52f)
                    lineTo(tl.x + side * 0.43f, tl.y + side * 0.70f)
                    lineTo(tl.x + side * 0.76f, tl.y + side * 0.32f)
                }
                drawPath(p, C.Black, style = Stroke(width = 2.4.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round, join = androidx.compose.ui.graphics.StrokeJoin.Round))
            } else {
                val w = 2.dp.toPx()
                drawRoundRect(C.Muted, Offset(tl.x + w / 2, tl.y + w / 2), Size(side - w, side - w), CornerRadius(4.dp.toPx()), style = Stroke(w))
            }
        }
        is Deco.Bar -> {
            val x = lay2.getHorizontalPosition(d.start.coerceIn(0, len), true)
            drawRoundRect(C.White, Offset(x, lineTop(d.start) + 3.dp.toPx()), Size(3.dp.toPx(), lineBottom(d.end) - lineTop(d.start) - 6.dp.toPx()), CornerRadius(2.dp.toPx()))
        }
        is Deco.Rule -> {
            val y = (lineTop(d.start) + lineBottom(d.start)) / 2f
            drawRect(C.Border, Offset(0f, y - 1.dp.toPx()), Size(size.width, LineW.toPx()))
        }
        is Deco.Code -> {}
    }
}

/** For each wrapped list item, quote or indented line: where its text starts, so later rows line up. */
private fun computeHangs(
    l: TextLayoutResult, lines: List<String>, infos: List<LineInfo>, starts: IntArray, hidden: BooleanArray, density: Density,
): Map<Int, TextUnit> {
    val len = l.layoutInput.text.length
    if (len != lines.sumOf { it.length + 1 } - 1) return emptyMap()
    val out = HashMap<Int, TextUnit>()
    for (i in lines.indices) {
        if (hidden[i]) continue
        val info = infos[i]
        val content = when (info.kind) {
            LineKind.BULLET, LineKind.NUMBER, LineKind.TASK, LineKind.QUOTE -> info.prefixEnd
            LineKind.TEXT -> lines[i].takeWhile { it == ' ' || it == '\t' }.length
            else -> 0
        }
        if (content <= 0 || content >= lines[i].length) continue
        val s = starts[i]
        val e = s + lines[i].length
        if (l.getLineForOffset(e) == l.getLineForOffset(s)) continue // doesn't wrap
        val px = l.getHorizontalPosition(s + content, true) - l.getHorizontalPosition(s, true)
        if (px <= 0f) continue
        out[i] = with(density) { (px / (this.density * fontScale)).sp }
    }
    return out
}

/** Small round chevron button: points right when folded, down when open. */
@Composable
fun FoldChevron(closed: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val angle by animateFloatAsState(if (closed) 0f else 90f, spring(stiffness = 700f), label = "chevron")
    Box(
        modifier.size(28.dp).clip(RoundedCornerShape(14.dp))
            .clickable(onClickLabel = if (closed) "Expand section" else "Collapse section", onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Chevron, null, tint = if (closed) C.White else C.Muted, modifier = Modifier.size(14.dp).rotate(angle))
    }
}

/* ------------------------------------------------------------------ */
/* Editing operations                                                  */
/* ------------------------------------------------------------------ */

object Ops {
    private fun lineBounds(t: String, pos: Int): IntRange {
        val s = t.lastIndexOf('\n', pos - 1) + 1
        val e = t.indexOf('\n', pos).let { if (it < 0) t.length else it }
        return s..e
    }

    /** Smart Enter: continue lists and quotes, or end them on an empty item. */
    fun afterChange(old: TextFieldValue, new: TextFieldValue): TextFieldValue {
        val o = old.text
        val n = new.text
        if (n.length != o.length + 1 || !old.selection.collapsed || !new.selection.collapsed) return new
        val at = old.selection.start
        if (at > o.length || n[at] != '\n' || new.selection.start != at + 1) return new
        if (n.substring(0, at) != o.substring(0, at)) return new
        val lb = lineBounds(o, at)
        val line = o.substring(lb.first, lb.last)
        if (at != lb.last) return new // only when pressing Enter at the end of a line
        val info = classifyLine(line)
        val content = if (info.prefixEnd <= line.length) line.substring(info.prefixEnd) else ""
        val prefix: String = when (info.kind) {
            LineKind.BULLET -> line.substring(0, info.prefixEnd).let { if (it.endsWith(" ")) it else "$it " }
            LineKind.TASK -> line.substring(0, info.markerStart) + info.number + " [ ] "
            LineKind.NUMBER -> {
                val num = info.number.dropLast(1).toIntOrNull() ?: 1
                line.substring(0, info.markerStart) + (num + 1) + info.number.last() + " "
            }
            LineKind.QUOTE -> "> "
            else -> return new
        }
        if (content.isBlank()) {
            // Empty item: remove the marker instead of continuing the list.
            val t = o.substring(0, lb.first) + o.substring(lb.last)
            return TextFieldValue(t, TextRange(lb.first))
        }
        val t = n.substring(0, at + 1) + prefix + n.substring(at + 1)
        return TextFieldValue(t, TextRange(at + 1 + prefix.length))
    }

    /** Apply [f] to every line touched by the selection. */
    private fun mapLines(v: TextFieldValue, f: (String) -> String): TextFieldValue {
        val t = v.text
        val a = minOf(v.selection.start, v.selection.end)
        val b = maxOf(v.selection.start, v.selection.end)
        val s = t.lastIndexOf('\n', a - 1) + 1
        val e = t.indexOf('\n', b).let { if (it < 0) t.length else it }
        val lines = t.substring(s, e).split('\n')
        val mapped = lines.map(f)
        val joined = mapped.joinToString("\n")
        val nt = t.substring(0, s) + joined + t.substring(e)
        return if (lines.size == 1) {
            val delta = mapped[0].length - lines[0].length
            val oldPrefix = classifyLine(lines[0]).prefixEnd
            fun shift(p: Int) = if (p - s >= oldPrefix) p + delta else minOf(s + classifyLine(mapped[0]).prefixEnd, s + mapped[0].length).coerceAtLeast(s)
            TextFieldValue(nt, TextRange(shift(v.selection.start), shift(v.selection.end)))
        } else {
            TextFieldValue(nt, TextRange(s, s + joined.length))
        }
    }

    private fun stripList(l: String): Pair<String, String> {
        val info = classifyLine(l)
        return when (info.kind) {
            LineKind.BULLET, LineKind.NUMBER, LineKind.TASK -> l.substring(0, info.indent) to l.substring(info.prefixEnd)
            else -> {
                val ind = l.takeWhile { it == ' ' || it == '\t' }
                ind to l.substring(ind.length)
            }
        }
    }

    fun heading(v: TextFieldValue) = mapLines(v) { l ->
        val info = classifyLine(l)
        val body = if (info.kind == LineKind.HEADING) l.substring(info.prefixEnd) else l.trimStart()
        when (if (info.kind == LineKind.HEADING) info.level else 0) {
            0 -> "# $body"
            1 -> "## $body"
            2 -> "### $body"
            else -> body
        }
    }

    fun bullet(v: TextFieldValue) = mapLines(v) { l ->
        if (classifyLine(l).kind == LineKind.BULLET) stripList(l).let { it.first + it.second }
        else stripList(l).let { it.first + "- " + it.second }
    }

    fun numbered(v: TextFieldValue): TextFieldValue {
        var n = 0
        return mapLines(v) { l ->
            if (classifyLine(l).kind == LineKind.NUMBER) stripList(l).let { it.first + it.second }
            else { n++; stripList(l).let { it.first + "$n. " + it.second } }
        }
    }

    fun task(v: TextFieldValue) = mapLines(v) { l ->
        val info = classifyLine(l)
        if (info.kind == LineKind.TASK) {
            val open = l.indexOf('[', info.markerStart)
            l.substring(0, open + 1) + (if (info.checked) ' ' else 'x') + l.substring(open + 2)
        } else stripList(l).let { it.first + "- [ ] " + it.second }
    }

    fun quote(v: TextFieldValue) = mapLines(v) { l ->
        val info = classifyLine(l)
        if (info.kind == LineKind.QUOTE) l.substring(info.prefixEnd) else "> $l"
    }

    fun indent(v: TextFieldValue) = mapLines(v) { "  $it" }

    fun outdent(v: TextFieldValue) = mapLines(v) { l ->
        when {
            l.startsWith("\t") -> l.substring(1)
            l.startsWith("  ") -> l.substring(2)
            l.startsWith(" ") -> l.substring(1)
            else -> l
        }
    }

    fun wrap(v: TextFieldValue, m: String): TextFieldValue {
        val t = v.text
        val a = minOf(v.selection.start, v.selection.end)
        val b = maxOf(v.selection.start, v.selection.end)
        val k = m.length
        // Unwrap if the selection (or empty cursor) already sits between markers.
        if (a >= k && b + k <= t.length && t.substring(a - k, a) == m && t.substring(b, b + k) == m) {
            val nt = t.substring(0, a - k) + t.substring(a, b) + t.substring(b + k)
            return TextFieldValue(nt, TextRange(a - k, b - k))
        }
        if (a == b) {
            // Wrap the word under the cursor, if any.
            var ws = a
            var we = a
            while (ws > 0 && t[ws - 1].isLetterOrDigit()) ws--
            while (we < t.length && t[we].isLetterOrDigit()) we++
            if (we > ws) {
                val nt = t.substring(0, ws) + m + t.substring(ws, we) + m + t.substring(we)
                return TextFieldValue(nt, TextRange(we + 2 * k))
            }
            val nt = t.substring(0, a) + m + m + t.substring(a)
            return TextFieldValue(nt, TextRange(a + k))
        }
        val nt = t.substring(0, a) + m + t.substring(a, b) + m + t.substring(b)
        return TextFieldValue(nt, TextRange(a + k, b + k))
    }

    fun link(v: TextFieldValue): TextFieldValue {
        val t = v.text
        val a = minOf(v.selection.start, v.selection.end)
        val b = maxOf(v.selection.start, v.selection.end)
        val label = t.substring(a, b)
        val nt = t.substring(0, a) + "[" + label + "](https://)" + t.substring(b)
        return if (label.isEmpty()) TextFieldValue(nt, TextRange(a + 1))
        else TextFieldValue(nt, TextRange(a + label.length + 3, a + label.length + 11))
    }

    fun rule(v: TextFieldValue): TextFieldValue {
        val t = v.text
        val e = t.indexOf('\n', v.selection.max).let { if (it < 0) t.length else it }
        val ins = "\n---\n"
        val nt = t.substring(0, e) + ins + t.substring(e)
        return TextFieldValue(nt, TextRange(e + ins.length))
    }

    /** Is the cursor inside a span of [kind] on its line? (lights up B / I) */
    fun inside(v: TextFieldValue, kind: InlineKind): Boolean {
        val t = v.text
        val p = v.selection.start.coerceIn(0, t.length)
        val lb = lineBounds(t, p)
        val spans = mutableListOf<InlineSpan>()
        scanInline(t, lb.first, lb.last, spans)
        return spans.any { it.kind == kind && p >= it.start && p <= it.end }
    }
}

/** Simple undo / redo over whole-document snapshots. */
class History(initial: TextFieldValue) {
    private val past = ArrayDeque<TextFieldValue>()
    private val future = ArrayDeque<TextFieldValue>()
    private var last = initial
    private var lastTime = 0L
    var version by mutableStateOf(0)
        private set

    val canUndo get() = past.isNotEmpty()
    val canRedo get() = future.isNotEmpty()

    fun record(prev: TextFieldValue, next: TextFieldValue, force: Boolean = false) {
        if (prev.text == next.text) { last = next; return }
        val now = System.currentTimeMillis()
        val boundary = force || now - lastTime > 1200 || next.text.length - prev.text.length != 1 ||
            next.text.getOrNull(prev.selection.start) == '\n' || next.text.getOrNull(prev.selection.start) == ' '
        if (boundary || past.isEmpty()) {
            past.addLast(prev)
            if (past.size > 200) past.removeFirst()
        }
        lastTime = now
        future.clear()
        last = next
        version++
    }

    fun undo(current: TextFieldValue): TextFieldValue? {
        val p = past.removeLastOrNull() ?: return null
        future.addLast(current)
        last = p
        lastTime = 0
        version++
        return p
    }

    fun redo(current: TextFieldValue): TextFieldValue? {
        val n = future.removeLastOrNull() ?: return null
        past.addLast(current)
        last = n
        lastTime = 0
        version++
        return n
    }
}

/* ------------------------------------------------------------------ */
/* Floating pill toolbar (option C)                                    */
/* ------------------------------------------------------------------ */

@Composable
fun FormatToolbar(value: TextFieldValue, onChange: (TextFieldValue) -> Unit, modifier: Modifier = Modifier) {
    var more by remember { mutableStateOf(false) }
    val bold = Ops.inside(value, InlineKind.BOLD)
    val italic = Ops.inside(value, InlineKind.ITALIC)
    Column(modifier.padding(horizontal = 10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        AnimatedVisibility(more, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
            Pill {
                ToolIcon(Icons.Outdent, "Outdent") { onChange(Ops.outdent(value)) }
                ToolText("S", "Strikethrough", strike = true) { onChange(Ops.wrap(value, "~~")) }
                ToolIcon(Icons.Code, "Code") { onChange(Ops.wrap(value, "`")) }
                ToolIcon(Icons.Link, "Link") { onChange(Ops.link(value)) }
                ToolIcon(Icons.Quote, "Quote") { onChange(Ops.quote(value)) }
                ToolIcon(Icons.Rule, "Divider") { onChange(Ops.rule(value)) }
            }
        }
        Pill {
            ToolText("H", "Heading") { onChange(Ops.heading(value)) }
            ToolText("B", "Bold", on = bold) { onChange(Ops.wrap(value, "**")) }
            ToolText("I", "Italic", italic = true, on = italic && !bold) { onChange(Ops.wrap(value, "*")) }
            ToolIcon(Icons.Bullets, "Bullet list") { onChange(Ops.bullet(value)) }
            ToolIcon(Icons.Numbers, "Numbered list") { onChange(Ops.numbered(value)) }
            ToolIcon(Icons.Checkbox, "Checkbox") { onChange(Ops.task(value)) }
            ToolIcon(Icons.Indent, "Indent") { onChange(Ops.indent(value)) }
            ToolIcon(Icons.MoreH, "More formatting", on = more) { more = !more }
        }
    }
}

/** Our own text toolbar: Compose asks it to show, and the editor shows [SelectionBar] instead of the
 *  system's floating menu, so cut, copy and paste always work and match the app. */
class NotedTextToolbar : androidx.compose.ui.platform.TextToolbar {
    var shown by mutableStateOf(false)
        private set
    override val status get() = if (shown) androidx.compose.ui.platform.TextToolbarStatus.Shown else androidx.compose.ui.platform.TextToolbarStatus.Hidden
    override fun showMenu(
        rect: Rect,
        onCopyRequested: (() -> Unit)?,
        onPasteRequested: (() -> Unit)?,
        onCutRequested: (() -> Unit)?,
        onSelectAllRequested: (() -> Unit)?,
    ) { shown = true }
    override fun hide() { shown = false }
}

/** Cut / Copy / Paste / Select all, shown above the formatting toolbar while text is selected. */
@Composable
fun SelectionBar(value: TextFieldValue, onChange: (TextFieldValue) -> Unit, onDone: () -> Unit, modifier: Modifier = Modifier) {
    val clipboard = LocalClipboardManager.current
    val t = value.text
    val a = value.selection.min.coerceIn(0, t.length)
    val b = value.selection.max.coerceIn(0, t.length)
    Column(modifier.padding(horizontal = 10.dp)) {
        Pill {
            if (b > a) {
                ToolText("Cut", "Cut") {
                    clipboard.setText(AnnotatedString(t.substring(a, b)))
                    onChange(TextFieldValue(t.substring(0, a) + t.substring(b), TextRange(a)))
                    onDone()
                }
                ToolText("Copy", "Copy") {
                    clipboard.setText(AnnotatedString(t.substring(a, b)))
                    onChange(value.copy(selection = TextRange(b)))
                    onDone()
                }
            }
            ToolText("Paste", "Paste") {
                val p = clipboard.getText()?.text
                if (!p.isNullOrEmpty()) onChange(TextFieldValue(t.substring(0, a) + p + t.substring(b), TextRange(a + p.length)))
                onDone()
            }
            ToolText("Select all", "Select all") { onChange(value.copy(selection = TextRange(0, t.length))) }
        }
    }
}

@Composable
private fun Pill(content: @Composable androidx.compose.foundation.layout.RowScope.() -> Unit) {
    Row(
        Modifier.fillMaxWidth().height(52.dp).clip(RoundedCornerShape(26.dp)).background(C.Toolbar).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        content = content,
    )
}

@Composable
private fun androidx.compose.foundation.layout.RowScope.ToolText(
    label: String, desc: String, italic: Boolean = false, strike: Boolean = false, on: Boolean = false, onClick: () -> Unit,
) {
    Box(
        Modifier.weight(1f).height(44.dp).clip(RoundedCornerShape(22.dp))
            .background(if (on) C.White else Color.Transparent)
            .clickable(onClickLabel = desc, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label, fontSize = 15.sp, fontWeight = if (italic) FontWeight.Normal else FontWeight.Bold,
            fontStyle = if (italic) FontStyle.Italic else FontStyle.Normal,
            textDecoration = if (strike) TextDecoration.LineThrough else null,
            color = if (on) C.Black else C.White,
        )
    }
}

@Composable
private fun androidx.compose.foundation.layout.RowScope.ToolIcon(icon: ImageVector, desc: String, on: Boolean = false, onClick: () -> Unit) {
    Box(
        Modifier.weight(1f).height(44.dp).clip(RoundedCornerShape(22.dp))
            .background(if (on) C.White else Color.Transparent)
            .clickable(onClickLabel = desc, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, desc, tint = if (on) C.Black else C.White, modifier = Modifier.size(20.dp))
    }
}
