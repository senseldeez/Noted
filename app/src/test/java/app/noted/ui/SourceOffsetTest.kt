package app.noted.ui

import org.junit.Assert.assertEquals
import org.junit.Test

/** Double-tapping rendered text in the reader must land the caret on the same character in the raw note. */
class SourceOffsetTest {
    private fun caretAt(src: String, block: Block, rendered: Int): Int = when (block) {
        is Block.Heading -> sourceOffset(src, block.line, block.text, rendered, SourceKind.PREFIXED)
        is Block.Paragraph -> sourceOffset(src, block.line, block.text, rendered, SourceKind.PARAGRAPH)
        is Block.Quote -> sourceOffset(src, block.line, block.text, rendered, SourceKind.PREFIXED)
        is Block.Code -> sourceOffset(src, block.line, block.text, rendered, SourceKind.CODE)
        else -> error("unsupported")
    }

    @Test fun headingSkipsHashes() {
        val src = "# Hello world"
        val b = parseBlocks(src)[0]
        // "Hello |world" -> after "# Hello "
        assertEquals(src.indexOf("world"), caretAt(src, b, 6))
    }

    @Test fun paragraphSkipsHiddenBold() {
        val src = "intro\n\nsome **bold** text\nsecond line"
        val b = parseBlocks(src)[1]
        // rendered "some bold text\nsecond line": offset of "text"
        assertEquals(src.indexOf("text"), caretAt(src, b, "some bold ".length))
        assertEquals(src.indexOf("line"), caretAt(src, b, "some bold text\nsecond ".length))
    }

    @Test fun nestedListItemAndContinuation() {
        val src = "- top\n  - [ ] child item\n    more"
        val list = parseBlocks(src)[0] as Block.ListBlock
        val child = list.items[0].children[0]
        assertEquals(src.indexOf("item"), sourceOffset(src, child.line, child.text, "child ".length, SourceKind.PREFIXED))
        assertEquals(src.indexOf("ore"), sourceOffset(src, child.line, child.text, "child item\nm".length, SourceKind.PREFIXED))
    }

    @Test fun quoteAndCode() {
        val src = "> quoted *it* here\n\n```\nval x = 1\n```"
        val blocks = parseBlocks(src)
        assertEquals(src.indexOf("here"), caretAt(src, blocks[0], "quoted it ".length))
        assertEquals(src.indexOf("= 1"), caretAt(src, blocks[1], "val x ".length))
    }
}
