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

fun headingSize(level: Int) = when (level) { 1 -> 24.sp; 2 -> 19.sp; 3 -> 16.sp; else -> 14.sp }

/** Character range [start, end) of the raw lines touched by the selection. */
fun activeRange(text: String, sel: TextRange): IntRange {
    val a = minOf(sel.start, sel.end).coerceIn(0, text.length)
    val b = maxOf(sel.start, sel.end).coerceIn(0, text.length)
    val s = text.lastIndexOf('\n', a - 1) + 1
    val e = text.indexOf('\n', b).let { if (it < 0) text.length else it }
    return s..e
}

class LivePreview(private val active: IntRange) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        val chars = raw.toCharArray()
        val base = mutableListOf<AnnotatedString.Range<SpanStyle>>()
        val top = mutableListOf<AnnotatedString.Range<SpanStyle>>()
        val lines = raw.split('\n')
        val infos = classifyLines(lines)
        var s = 0
        for ((i, l) in lines.withIndex()) {
            val e = s + l.length
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
                LineKind.FENCE -> style(Dim, s, e)
                LineKind.CODE -> style(SpanStyle(color = C.Text2), s, e)
                LineKind.TEXT -> {
                    style(SpanStyle(color = C.Text2), s, e)
                    inline(s)
                }
                LineKind.BLANK -> {}
            }
            s = e + 1
        }
        return TransformedText(AnnotatedString(String(chars), base + top), OffsetMapping.Identity)
    }

    override fun equals(other: Any?) = other is LivePreview && other.active == active
    override fun hashCode() = active.hashCode()
}

private sealed class Deco {
    data class Box(val open: Int, val checked: Boolean) : Deco()
    data class Bar(val start: Int, val end: Int) : Deco()
    data class Rule(val start: Int) : Deco()
    data class Code(val start: Int, val end: Int) : Deco()
}

private fun decorations(raw: String, active: IntRange): List<Deco> {
    val out = mutableListOf<Deco>()
    val lines = raw.split('\n')
    val infos = classifyLines(lines)
    var s = 0
    var fenceStart = -1
    for ((i, l) in lines.withIndex()) {
        val e = s + l.length
        val isActive = s <= active.last && e >= active.first
        val info = infos[i]
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
    modifier: Modifier = Modifier,
) {
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val active = activeRange(value.text, value.selection)
    val transformation = remember(active) { LivePreview(active) }
    val decos = remember(value.text, active) { decorations(value.text, active) }
    val textStyle = remember {
        TextStyle(fontFamily = Mono, fontSize = 14.sp, lineHeight = 22.sp, color = C.White)
    }
    Box(
        modifier.drawBehind {
            val lay = layout ?: return@drawBehind
            val len = lay.layoutInput.text.length
            fun lineTop(off: Int) = lay.getLineTop(lay.getLineForOffset(off.coerceIn(0, len)))
            fun lineBottom(off: Int) = lay.getLineBottom(lay.getLineForOffset(off.coerceIn(0, len)))
            // Code blocks.
            for (d in decos) if (d is Deco.Code) {
                drawRoundRect(
                    C.CodeBg, topLeft = Offset(-10.dp.toPx(), lineTop(d.start) - 4.dp.toPx()),
                    size = Size(size.width + 20.dp.toPx(), lineBottom(d.end) - lineTop(d.start) + 8.dp.toPx()),
                    cornerRadius = CornerRadius(14.dp.toPx()),
                )
            }
            // The line being edited: a rounded dark-grey pill.
            if (active.first <= len) {
                val raw = value.text
                var first = active.first
                while (first < active.last && first < raw.length && (raw[first] == ' ' || raw[first] == '\t')) first++
                val x = if (first < len) lay.getHorizontalPosition(first, true) else 0f
                val top = lineTop(active.first) - 4.dp.toPx()
                val bottom = lineBottom(active.last) + 4.dp.toPx()
                drawRoundRect(
                    C.Pill, topLeft = Offset(x - 12.dp.toPx(), top),
                    size = Size(size.width - x + 22.dp.toPx(), bottom - top),
                    cornerRadius = CornerRadius(14.dp.toPx()),
                )
            }
            for (d in decos) when (d) {
                is Deco.Box -> if (d.open + 2 < len) {
                    val a = lay.getBoundingBox(d.open)
                    val b = lay.getBoundingBox(d.open + 2)
                    val side = 15.dp.toPx()
                    val cx = (a.left + b.right) / 2f
                    val cy = (a.top + a.bottom) / 2f
                    val tl = Offset(cx - side / 2, cy - side / 2)
                    if (d.checked) {
                        drawRoundRect(C.White, tl, Size(side, side), CornerRadius(4.dp.toPx()))
                        val p = androidx.compose.ui.graphics.Path().apply {
                            moveTo(tl.x + side * 0.25f, tl.y + side * 0.52f)
                            lineTo(tl.x + side * 0.43f, tl.y + side * 0.70f)
                            lineTo(tl.x + side * 0.76f, tl.y + side * 0.32f)
                        }
                        drawPath(p, C.Black, style = Stroke(width = 2.dp.toPx(), cap = androidx.compose.ui.graphics.StrokeCap.Round, join = androidx.compose.ui.graphics.StrokeJoin.Round))
                    } else {
                        val w = 1.5.dp.toPx()
                        drawRoundRect(
                            C.Muted, Offset(tl.x + w / 2, tl.y + w / 2), Size(side - w, side - w),
                            CornerRadius(4.dp.toPx()), style = Stroke(w),
                        )
                    }
                }
                is Deco.Bar -> {
                    val x = lay.getHorizontalPosition(d.start.coerceIn(0, len), true)
                    drawRect(C.White, Offset(x, lineTop(d.start)), Size(2.dp.toPx(), lineBottom(d.end) - lineTop(d.start)))
                }
                is Deco.Rule -> {
                    val y = (lineTop(d.start) + lineBottom(d.start)) / 2f
                    drawRect(C.Border, Offset(0f, y), Size(size.width, 1.dp.toPx()))
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
        if (more) {
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
