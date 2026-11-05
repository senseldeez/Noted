package app.noted.ui

import androidx.compose.foundation.background
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

/** Heading sizes are capped so every glyph fits inside the shared 28sp line box. */
fun headingSize(level: Int) = when (level) { 1 -> 22.sp; 2 -> 19.sp; 3 -> 17.sp; else -> 15.sp }

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

class LivePreview(private val active: IntRange, private val folded: Set<String>) : VisualTransformation {
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
                LineKind.CODE -> style(SpanStyle(color = C.Text2, fontFamily = Mono, fontSize = 13.sp), s, e)
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

    override fun equals(other: Any?) = other is LivePreview && other.active == active && other.folded == folded
    override fun hashCode() = active.hashCode() * 31 + folded.hashCode()
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

@Composable
fun LiveEditor(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    focus: FocusRequester,
    folded: Set<String>,
    onToggleFold: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val active = activeRange(value.text, value.selection)
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
    val transformation = remember(active, effectiveFolded) { LivePreview(active, effectiveFolded) }
    val decos = remember(value.text, active, effectiveFolded) { decorations(value.text, active, effectiveFolded) }
    val textStyle = remember {
        TextStyle(fontFamily = Sans, fontSize = 15.sp, lineHeight = BodyLine, lineHeightStyle = CenteredLines, color = C.White)
    }
    val density = LocalDensity.current

    // Where the active-line pill should sit; animated so it glides between lines.
    val lay = layout
    var pillTarget = Rect.Zero
    if (lay != null) {
        val len = lay.layoutInput.text.length
        if (active.first <= len) {
            val raw = value.text
            var first = active.first
            while (first < active.last && first < raw.length && (raw[first] == ' ' || raw[first] == '\t')) first++
            val x = if (first < len) lay.getHorizontalPosition(first, true) else 0f
            val top = lay.getLineTop(lay.getLineForOffset(active.first.coerceIn(0, len)))
            val bottom = lay.getLineBottom(lay.getLineForOffset(active.last.coerceIn(0, len)))
            pillTarget = Rect(x, top, 0f, bottom)
        }
    }
    val spec = spring<Float>(stiffness = 900f)
    val pillTop by animateFloatAsState(pillTarget.top, spec, label = "pillTop")
    val pillBottom by animateFloatAsState(pillTarget.bottom, spec, label = "pillBottom")
    val pillX by animateFloatAsState(pillTarget.left, spec, label = "pillX")

    Box(
        modifier.drawBehind {
            val lay2 = layout ?: return@drawBehind
            val len = lay2.layoutInput.text.length
            fun lineTop(off: Int) = lay2.getLineTop(lay2.getLineForOffset(off.coerceIn(0, len)))
            fun lineBottom(off: Int) = lay2.getLineBottom(lay2.getLineForOffset(off.coerceIn(0, len)))
            // Code blocks.
            for (d in decos) if (d is Deco.Code) {
                drawRoundRect(
                    C.CodeBg, topLeft = Offset(-10.dp.toPx(), lineTop(d.start)),
                    size = Size(size.width + 20.dp.toPx(), lineBottom(d.end) - lineTop(d.start)),
                    cornerRadius = CornerRadius(14.dp.toPx()),
                )
                drawRoundRect(
                    C.Guide, topLeft = Offset(-10.dp.toPx(), lineTop(d.start)),
                    size = Size(size.width + 20.dp.toPx(), lineBottom(d.end) - lineTop(d.start)),
                    cornerRadius = CornerRadius(14.dp.toPx()), style = Stroke(LineW.toPx()),
                )
            }
            // The line being edited: a rounded dark-grey pill that stays inside its own line box.
            if (active.first <= len && pillBottom > pillTop) {
                val inset = 1.dp.toPx()
                drawRoundRect(
                    C.Pill, topLeft = Offset(pillX - 12.dp.toPx(), pillTop + inset),
                    size = Size(size.width - pillX + 22.dp.toPx(), pillBottom - pillTop - 2 * inset),
                    cornerRadius = CornerRadius(14.dp.toPx()),
                )
            }
            for (d in decos) when (d) {
                is Deco.Box -> if (d.open + 2 < len) {
                    val a = lay2.getBoundingBox(d.open)
                    val b = lay2.getBoundingBox(d.open + 2)
                    val side = 16.dp.toPx()
                    val cx = (a.left + b.right) / 2f
                    val cy = (lineTop(d.open) + lineBottom(d.open)) / 2f
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
                        drawRoundRect(
                            C.Muted, Offset(tl.x + w / 2, tl.y + w / 2), Size(side - w, side - w),
                            CornerRadius(4.dp.toPx()), style = Stroke(w),
                        )
                    }
                }
                is Deco.Bar -> {
                    val x = lay2.getHorizontalPosition(d.start.coerceIn(0, len), true)
                    drawRoundRect(
                        C.White, Offset(x, lineTop(d.start) + 3.dp.toPx()),
                        Size(3.dp.toPx(), lineBottom(d.end) - lineTop(d.start) - 6.dp.toPx()), CornerRadius(2.dp.toPx()),
                    )
                }
                is Deco.Rule -> {
                    val y = (lineTop(d.start) + lineBottom(d.start)) / 2f
                    drawRect(C.Border, Offset(0f, y - 1.dp.toPx()), Size(size.width, LineW.toPx()))
                }
                is Deco.Code -> {}
            }
        },
    ) {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            textStyle = textStyle,
            visualTransformation = transformation,
            onTextLayout = { layout = it },
            cursorBrush = SolidColor(C.White),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, autoCorrectEnabled = true),
            modifier = Modifier.fillMaxWidth().focusRequester(focus),
        )
        // Fold chevrons in the left gutter, one per heading that has content.
        val l = layout
        if (l != null) {
            val len = l.layoutInput.text.length
            val hidden = remember(lines, infos, effectiveFolded) { foldedLines(lines, infos, effectiveFolded) }
            for ((h, _) in sections) {
                if (hidden[h]) continue
                val start = lineStarts[h].coerceIn(0, len)
                val ln = l.getLineForOffset(start)
                val top = l.getLineTop(ln)
                val mid = (top + l.getLineBottom(ln)) / 2f
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
    }
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
