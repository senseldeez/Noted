package app.noted.ui

/*
 * Blocks (a line, a heading with its section, a list item with its children, a code block)
 * that can be dragged around the editor, and merging a note that changed on disk while open.
 * The desktop app (desktop/src/blocks.rs) does the same.
 */

/** Lines [first, last) of a block or piece of text. */
data class LineSpan(val first: Int, val last: Int) {
    val size get() = last - first
    operator fun contains(i: Int) = i in first until last
}

private fun indentOf(l: String): Int {
    var n = 0
    for (c in l) {
        when (c) {
            ' ' -> n++
            '\t' -> n += 4
            else -> return n
        }
    }
    return n
}

/** Fenced code blocks, from the opening fence through the closing one. */
private fun fences(infos: List<LineInfo>): List<LineSpan> {
    val out = mutableListOf<LineSpan>()
    var open = -1
    for (i in infos.indices) if (infos[i].kind == LineKind.FENCE) {
        if (open < 0) open = i else { out += LineSpan(open, i + 1); open = -1 }
    }
    if (open >= 0) out += LineSpan(open, infos.size)
    return out
}

/** The lines making up the block that line [i] belongs to. */
fun blockAt(lines: List<String>, i0: Int, infos: List<LineInfo> = classifyLines(lines)): LineSpan {
    if (lines.isEmpty()) return LineSpan(0, 0)
    val i = i0.coerceIn(0, lines.size - 1)
    val info = infos[i]
    if (info.kind == LineKind.FENCE || info.kind == LineKind.CODE) {
        fences(infos).firstOrNull { i in it }?.let { return it }
    }
    return when (info.kind) {
        LineKind.HEADING -> {
            var j = i + 1
            while (j < lines.size && !(infos[j].kind == LineKind.HEADING && infos[j].level <= info.level)) j++
            LineSpan(i, j)
        }
        LineKind.BULLET, LineKind.NUMBER, LineKind.TASK -> {
            val ind = indentOf(lines[i])
            var j = i + 1
            while (j < lines.size) {
                val l = lines[j]
                if (l.isBlank() || indentOf(l) <= ind || infos[j].kind == LineKind.HEADING) break
                j++
            }
            LineSpan(i, j)
        }
        else -> LineSpan(i, i + 1)
    }
}

/**
 * The other lines, grouped into the pieces that move as one while [skip] is dragged:
 * code blocks and folded sections stay together, everything else is a line on its own.
 */
fun dragUnits(lines: List<String>, skip: LineSpan, hidden: BooleanArray, infos: List<LineInfo> = classifyLines(lines)): List<LineSpan> {
    val code = fences(infos)
    val out = mutableListOf<LineSpan>()
    var i = 0
    while (i < lines.size) {
        if (i in skip) { i = skip.last; continue }
        var j = i + 1
        code.firstOrNull { it.first == i }?.let { j = maxOf(j, it.last) }
        while (j < lines.size && hidden.getOrElse(j) { false } && j !in skip) j++
        out += LineSpan(i, j)
        i = j
    }
    return out
}

/** The new text, the line the block now starts on, and for each new line the old line it came from. */
data class Moved(val text: String, val at: Int, val order: List<Int>)

/** Move [block] so it sits before unit [gap] of [units]. */
fun moveBlock(lines: List<String>, block: LineSpan, units: List<LineSpan>, gap: Int): Moved {
    val order = ArrayList<Int>(lines.size)
    var at = 0
    for ((k, u) in units.withIndex()) {
        if (k == gap) { at = order.size; for (x in block.first until block.last) order += x }
        for (x in u.first until u.last) order += x
    }
    if (gap >= units.size) { at = order.size; for (x in block.first until block.last) order += x }
    return Moved(order.joinToString("\n") { lines[it] }, at, order)
}

/** The unit index the block sits before right now (dropping there changes nothing). */
fun homeGap(block: LineSpan, units: List<LineSpan>): Int =
    units.indexOfFirst { it.first >= block.last }.let { if (it < 0) units.size else it }

/** Swap the block one unit up or down; null when it is already at that end. */
fun nudgeBlock(lines: List<String>, block: LineSpan, hidden: BooleanArray, down: Boolean): Moved? {
    val us = dragUnits(lines, block, hidden)
    val home = homeGap(block, us)
    val gap = if (down) { if (home >= us.size) return null; home + 1 } else { if (home == 0) return null; home - 1 }
    return moveBlock(lines, block, us, gap)
}

/** Where char [c] of the old text lands after its lines were reordered (`order[new] = old`). */
fun remapOffset(lines: List<String>, order: List<Int>, c: Int): Int {
    var s = 0
    var li = 0
    for (i in lines.indices) {
        if (c < s + lines[i].length + 1 || i == lines.size - 1) { li = i; break }
        s += lines[i].length + 1
    }
    val col = c - s
    var acc = 0
    for (o in order) {
        if (o == li) return acc + col.coerceIn(0, lines[o].length)
        acc += lines[o].length + 1
    }
    return c
}

/* ---------- merging a note that changed on disk ---------- */

private fun span(base: String, new: String): Pair<Int, Int> {
    var p = 0
    val n = minOf(base.length, new.length)
    while (p < n && base[p] == new[p]) p++
    var s = 0
    while (s < n - p && base[base.length - 1 - s] == new[new.length - 1 - s]) s++
    return p to s
}

/**
 * Three-way merge of a note: [base] is what we last loaded or saved, [ours] the text in the
 * editor, [theirs] what is on disk now. Edits in different places both survive; when they
 * overlap ours wins (it is saved right after). Also maps [cursor] (an offset into ours).
 */
fun mergeNote(base: String, ours: String, theirs: String, cursor: Int): Pair<String, Int> {
    val (tp, ts) = span(base, theirs)
    val t0 = tp
    val t1 = base.length - ts
    val tNew = theirs.substring(tp, theirs.length - ts)
    val tDelta = (theirs.length - ts) - t1
    if (ours == base) {
        val c = when {
            cursor <= t0 -> cursor
            cursor >= t1 -> cursor + tDelta
            else -> tp + tNew.length
        }
        return theirs to c.coerceIn(0, theirs.length)
    }
    val (op, os) = span(base, ours)
    val o0 = op
    val o1 = base.length - os
    if (o1 <= t0 && !(o0 == o1 && o1 == t0 && t0 == t1)) {
        val out = ours.substring(0, ours.length - os) + base.substring(o1, t0) + tNew + base.substring(t1)
        val c = if (cursor <= ours.length - os) cursor else cursor + tDelta
        return out to c.coerceIn(0, out.length)
    }
    if (t1 <= o0) {
        val out = base.substring(0, t0) + tNew + base.substring(t1, o0) + ours.substring(op)
        val c = if (cursor <= t0) cursor else cursor + tDelta
        return out to c.coerceIn(0, out.length)
    }
    return ours to cursor
}
