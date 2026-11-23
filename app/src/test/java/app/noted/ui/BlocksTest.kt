package app.noted.ui

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import androidx.compose.ui.text.input.TextFieldValue

class BlocksTest {
    private fun ls(s: String) = s.split('\n')

    @Test fun blocks() {
        val l = ls("# A\ntext\n- one\n  - kid\n- two\n```\ncode\n```\n# B\nx")
        assertEquals(LineSpan(0, 8), blockAt(l, 0))
        assertEquals(LineSpan(1, 2), blockAt(l, 1))
        assertEquals(LineSpan(2, 4), blockAt(l, 2))
        assertEquals(LineSpan(3, 4), blockAt(l, 3))
        assertEquals(LineSpan(5, 8), blockAt(l, 6))
        assertEquals(LineSpan(5, 8), blockAt(l, 7))
        assertEquals(LineSpan(8, 10), blockAt(l, 8))
    }

    @Test fun moving() {
        val l = ls("a\n- one\n  - kid\nb\nc")
        val blk = blockAt(l, 1)
        val us = dragUnits(l, blk, BooleanArray(l.size))
        assertEquals(listOf(LineSpan(0, 1), LineSpan(3, 4), LineSpan(4, 5)), us)
        assertEquals(1, homeGap(blk, us))
        assertEquals("a\nb\nc\n- one\n  - kid", moveBlock(l, blk, us, 3).text)
        assertEquals(listOf(0, 3, 4, 1, 2), moveBlock(l, blk, us, 3).order)
        assertEquals("a\nb\n- one\n  - kid\nc", nudgeBlock(l, blk, BooleanArray(l.size), true)!!.text)
        assertNull(nudgeBlock(ls("x\ny"), LineSpan(0, 1), BooleanArray(2), false))
        // The cursor on "b" (offset 16) follows it to line 1.
        assertEquals(2, remapOffset(l, moveBlock(l, blk, us, 3).order, 16))
    }

    @Test fun merging() {
        assertEquals("hello big world" to 15, mergeNote("hello world", "hello world", "hello big world", 11))
        assertEquals("hello big world" to 2, mergeNote("hello world", "hello world", "hello big world", 2))
        assertEquals("aaa!\nbbb\nccc?", mergeNote("aaa\nbbb\nccc", "aaa!\nbbb\nccc", "aaa\nbbb\nccc?", 4).first)
        assertEquals("aaa!\nbbb\nccc?" to 13, mergeNote("aaa\nbbb\nccc", "aaa\nbbb\nccc?", "aaa!\nbbb\nccc", 12))
        assertEquals("aXc", mergeNote("abc", "aXc", "aYc", 2).first)
    }

    @Test fun pasting() {
        val v = TextFieldValue("a\n- one\n  - kid\nb")
        val r = applyBlockOp(v, 1, BlockOp.PASTE, emptySet(), "x\ny\n") {}!!
        assertEquals("a\n- one\n  - kid\nx\ny\nb", r.text)
        assertEquals(19, r.selection.start)
        assertNull(applyBlockOp(v, 1, BlockOp.PASTE, emptySet(), "") {})
    }
}
