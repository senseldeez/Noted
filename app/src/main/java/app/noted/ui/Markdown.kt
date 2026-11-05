package app.noted.ui

/* Small, forgiving markdown parser shared by the reader, the live-preview editor and card previews. */

enum class InlineKind { BOLD, ITALIC, STRIKE, CODE, LINK, MARK, URL }

/** A styled range of a line. MARK and URL ranges are syntax (hidden when rendered). */
data class InlineSpan(val start: Int, val end: Int, val kind: InlineKind, val url: String? = null)

private val LINK_RE = Regex("""\[([^\]\n]+)]\(([^)\s]+)\)""")

fun scanInline(t: String, from: Int, to: Int, out: MutableList<InlineSpan>) {
    var i = from
    while (i < to) {
        val c = t[i]
        if (c == '\\' && i + 1 < to && t[i + 1] in "\\`*_~[]()#>-+!") {
            out += InlineSpan(i, i + 1, InlineKind.MARK)
            i += 2
            continue
        }
        if (c == '`') {
            val e = t.indexOf('`', i + 1)
            if (e in (i + 2) until to) {
                out += InlineSpan(i, i + 1, InlineKind.MARK)
                out += InlineSpan(i + 1, e, InlineKind.CODE)
                out += InlineSpan(e, e + 1, InlineKind.MARK)
                i = e + 1
                continue
            }
        }
        if ((c == '*' || c == '_' || c == '~') && i + 1 < to && t[i + 1] == c) {
            val m = "$c$c"
            val e = t.indexOf(m, i + 2)
            if (e >= i + 3 && e + 2 <= to && t[i + 2] != ' ' && t[e - 1] != ' ') {
                val kind = if (c == '~') InlineKind.STRIKE else InlineKind.BOLD
                out += InlineSpan(i, i + 2, InlineKind.MARK)
                out += InlineSpan(i + 2, e, kind)
                scanInline(t, i + 2, e, out)
                out += InlineSpan(e, e + 2, InlineKind.MARK)
                i = e + 2
                continue
            }
        }
        if ((c == '*' || c == '_') && i + 1 < to && t[i + 1] != ' ' && t[i + 1] != c &&
            !(c == '_' && i > 0 && t[i - 1].isLetterOrDigit())
        ) {
            var e = t.indexOf(c, i + 1)
            while (e in (i + 1) until to && e + 1 < to && t[e + 1] == c) e = t.indexOf(c, e + 2)
            if (e in (i + 2) until to && t[e - 1] != ' ' && !(c == '_' && e + 1 < to && t[e + 1].isLetterOrDigit())) {
                out += InlineSpan(i, i + 1, InlineKind.MARK)
                out += InlineSpan(i + 1, e, InlineKind.ITALIC)
                scanInline(t, i + 1, e, out)
                out += InlineSpan(e, e + 1, InlineKind.MARK)
                i = e + 1
                continue
            }
        }
        if (c == '[') {
            val m = LINK_RE.find(t, i)
            if (m != null && m.range.first == i && m.range.last < to) {
                val textEnd = m.groups[1]!!.range.last + 1
                out += InlineSpan(i, i + 1, InlineKind.MARK)
                out += InlineSpan(i + 1, textEnd, InlineKind.LINK, m.groupValues[2])
                scanInline(t, i + 1, textEnd, out)
                out += InlineSpan(textEnd, m.range.last + 1, InlineKind.URL)
                i = m.range.last + 1
                continue
            }
        }
        i++
    }
}

enum class LineKind { TEXT, HEADING, BULLET, NUMBER, TASK, QUOTE, RULE, FENCE, CODE, BLANK }

/**
 * One raw line. [indent] = leading whitespace length, [prefixEnd] = index where the
 * line's content starts (after "# ", "- ", "- [ ] ", "> " ...).
 */
data class LineInfo(
    val kind: LineKind,
    val indent: Int = 0,
    val markerStart: Int = 0,
    val prefixEnd: Int = 0,
    val level: Int = 0,
    val checked: Boolean = false,
    val number: String = "",
)

private val HEADING_RE = Regex("""^(#{1,6})[ \t]+""")
private val TASK_RE = Regex("""^([ \t]*)([-*+]|\d+[.)])[ \t]+\[([ xX])](?:[ \t]+|$)""")
private val BULLET_RE = Regex("""^([ \t]*)([-*+])(?:[ \t]+|$)""")
private val NUMBER_RE = Regex("""^([ \t]*)(\d+[.)])(?:[ \t]+|$)""")
private val QUOTE_RE = Regex("""^[ \t]*>[ \t]?""")
private val RULE_RE = Regex("""^[ \t]*([-*_])([ \t]*\1){2,}[ \t]*$""")
private val FENCE_RE = Regex("""^[ \t]*(```|~~~)""")

fun indentWidth(s: String, n: Int): Int {
    var w = 0
    for (k in 0 until n) w += if (s[k] == '\t') 4 else 1
    return w
}

/** Classify every line, tracking fenced code blocks. */
fun classifyLines(lines: List<String>): List<LineInfo> {
    var inFence = false
    return lines.map { l ->
        if (FENCE_RE.containsMatchIn(l)) {
            inFence = !inFence
            return@map LineInfo(LineKind.FENCE)
        }
        if (inFence) return@map LineInfo(LineKind.CODE)
        classifyLine(l)
    }
}

fun classifyLine(l: String): LineInfo {
    if (l.isBlank()) return LineInfo(LineKind.BLANK)
    HEADING_RE.find(l)?.let { return LineInfo(LineKind.HEADING, prefixEnd = it.range.last + 1, level = it.groupValues[1].length) }
    if (RULE_RE.matches(l)) return LineInfo(LineKind.RULE)
    TASK_RE.find(l)?.let {
        val ind = it.groupValues[1].length
        return LineInfo(
            LineKind.TASK, indent = ind, markerStart = ind, prefixEnd = it.range.last + 1,
            checked = it.groupValues[3] != " ", number = it.groupValues[2],
        )
    }
    BULLET_RE.find(l)?.let {
        val ind = it.groupValues[1].length
        return LineInfo(LineKind.BULLET, indent = ind, markerStart = ind, prefixEnd = it.range.last + 1)
    }
    NUMBER_RE.find(l)?.let {
        val ind = it.groupValues[1].length
        return LineInfo(LineKind.NUMBER, indent = ind, markerStart = ind, prefixEnd = it.range.last + 1, number = it.groupValues[2])
    }
    QUOTE_RE.find(l)?.let { return LineInfo(LineKind.QUOTE, markerStart = it.range.first, prefixEnd = it.range.last + 1) }
    return LineInfo(LineKind.TEXT)
}

/* ---------- Reader blocks ---------- */

sealed class Block {
    data class Heading(val level: Int, val text: String, val raw: String) : Block()
    data class Paragraph(val text: String) : Block()
    data class Quote(val text: String) : Block()
    data class Code(val text: String) : Block()
    data object Rule : Block()
    data class ListBlock(val items: List<ListNode>) : Block()
}

class ListNode(
    val line: Int,
    val info: LineInfo,
    var text: String,
    val children: MutableList<ListNode> = mutableListOf(),
)

fun parseBlocks(src: String): List<Block> {
    val lines = src.split('\n')
    val infos = classifyLines(lines)
    val out = mutableListOf<Block>()
    var i = 0
    while (i < lines.size) {
        val info = infos[i]
        val l = lines[i]
        when (info.kind) {
            LineKind.BLANK -> i++
            LineKind.HEADING -> { out += Block.Heading(info.level, l.substring(info.prefixEnd).trimEnd().trimEnd('#').trimEnd(), headingKey(l)); i++ }
            LineKind.RULE -> { out += Block.Rule; i++ }
            LineKind.FENCE -> {
                val sb = StringBuilder()
                i++
                while (i < lines.size && infos[i].kind == LineKind.CODE) {
                    if (sb.isNotEmpty()) sb.append('\n')
                    sb.append(lines[i]); i++
                }
                if (i < lines.size && infos[i].kind == LineKind.FENCE) i++
                out += Block.Code(sb.toString())
            }
            LineKind.CODE -> i++
            LineKind.QUOTE -> {
                val sb = StringBuilder()
                while (i < lines.size && infos[i].kind == LineKind.QUOTE) {
                    if (sb.isNotEmpty()) sb.append('\n')
                    sb.append(lines[i].substring(infos[i].prefixEnd)); i++
                }
                out += Block.Quote(sb.toString())
            }
            LineKind.BULLET, LineKind.NUMBER, LineKind.TASK -> {
                val roots = mutableListOf<ListNode>()
                val stack = ArrayDeque<Pair<Int, ListNode>>() // indent width -> node
                while (i < lines.size) {
                    val inf = infos[i]
                    val isItem = inf.kind == LineKind.BULLET || inf.kind == LineKind.NUMBER || inf.kind == LineKind.TASK
                    if (!isItem) {
                        // An indented plain line continues the previous item.
                        if (inf.kind == LineKind.TEXT && lines[i].startsWith("  ") && stack.isNotEmpty()) {
                            val last = stack.last().second
                            last.text = last.text + "\n" + lines[i].trim()
                            i++; continue
                        }
                        break
                    }
                    val w = indentWidth(lines[i], inf.indent)
                    val node = ListNode(i, inf, lines[i].substring(inf.prefixEnd))
                    while (stack.isNotEmpty() && stack.last().first >= w) stack.removeLast()
                    if (stack.isEmpty()) roots += node else stack.last().second.children += node
                    stack.addLast(w to node)
                    i++
                }
                out += Block.ListBlock(roots)
            }
            LineKind.TEXT -> {
                val sb = StringBuilder()
                while (i < lines.size && infos[i].kind == LineKind.TEXT) {
                    if (sb.isNotEmpty()) sb.append('\n')
                    sb.append(lines[i].trimEnd()); i++
                }
                out += Block.Paragraph(sb.toString())
            }
        }
    }
    return out
}

/** Flip the checkbox on one raw line. */
fun toggleTaskLine(src: String, line: Int): String {
    val lines = src.split('\n').toMutableList()
    if (line !in lines.indices) return src
    val l = lines[line]
    val info = classifyLine(l)
    if (info.kind != LineKind.TASK) return src
    val open = l.indexOf('[', info.markerStart)
    if (open < 0) return src
    val ch = if (info.checked) ' ' else 'x'
    lines[line] = l.substring(0, open + 1) + ch + l.substring(open + 2)
    return lines.joinToString("\n")
}

fun wordCount(s: String): Int = s.split(Regex("\\s+")).count { it.any(Char::isLetterOrDigit) }
