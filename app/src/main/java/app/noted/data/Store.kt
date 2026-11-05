package app.noted.data

import org.json.JSONArray
import org.json.JSONObject
import java.io.File

/** One card in the grid: a note (.md file) or a real folder on disk. */
data class Entry(
    val file: File,
    val isFolder: Boolean,
    val pinned: Boolean,
    val modified: Long,
    /** Notes: the first lines of the file. Folders: empty. */
    val preview: String = "",
    /** Folders: names of a few children. */
    val children: List<String> = emptyList(),
    val childCount: Int = 0,
) {
    val key: String get() = file.absolutePath
    val title: String get() = if (isFolder) file.name else file.name.removeMdExtension()
}

fun String.removeMdExtension(): String = when {
    endsWith(".md", true) -> dropLast(3)
    endsWith(".markdown", true) -> dropLast(9)
    else -> this
}

fun File.isNote(): Boolean = isFile && !name.startsWith(".") &&
    (name.endsWith(".md", true) || name.endsWith(".markdown", true))

fun File.isVisibleFolder(): Boolean = isDirectory && !name.startsWith(".")

/**
 * Pins and manual order live in a small hidden file inside each folder, so they
 * travel with the folder through any sync app and survive renames of parents.
 */
class FolderMeta(val pinned: MutableList<String> = mutableListOf(), val order: MutableList<String> = mutableListOf())

object Store {
    private const val META = ".noted.json"

    fun readMeta(dir: File): FolderMeta {
        val f = File(dir, META)
        if (!f.exists()) return FolderMeta()
        return try {
            val o = JSONObject(f.readText())
            FolderMeta(o.optJSONArray("pinned").toList(), o.optJSONArray("order").toList())
        } catch (e: Exception) {
            FolderMeta()
        }
    }

    fun writeMeta(dir: File, meta: FolderMeta) {
        val o = JSONObject()
        o.put("pinned", JSONArray(meta.pinned.distinct()))
        o.put("order", JSONArray(meta.order.distinct()))
        try {
            File(dir, META).writeText(o.toString(2))
        } catch (_: Exception) {
        }
    }

    private fun JSONArray?.toList(): MutableList<String> {
        val out = mutableListOf<String>()
        if (this == null) return out
        for (i in 0 until length()) out += optString(i)
        return out
    }

    private fun visibleChildren(dir: File): List<File> =
        dir.listFiles()?.filter { it.isNote() || it.isVisibleFolder() } ?: emptyList()

    /** Pinned items first (in pin order), then new unordered items (newest first), then the manual order. */
    fun list(dir: File): List<Entry> {
        val meta = readMeta(dir)
        val files = visibleChildren(dir)
        val entries = files.map { f ->
            if (f.isDirectory) {
                val kids = visibleChildren(f).sortedByDescending { it.lastModified() }
                Entry(
                    file = f, isFolder = true, pinned = f.name in meta.pinned, modified = f.lastModified(),
                    children = kids.take(3).map { if (it.isDirectory) it.name + "/" else it.name },
                    childCount = kids.size,
                )
            } else {
                Entry(file = f, isFolder = false, pinned = f.name in meta.pinned, modified = f.lastModified(), preview = readHead(f))
            }
        }
        val byName = entries.associateBy { it.file.name }
        val pinned = meta.pinned.mapNotNull { byName[it] }
        val rest = entries.filter { !it.pinned }
        val ordered = meta.order.mapNotNull { byName[it] }.filter { !it.pinned }
        val unordered = rest.filter { it.file.name !in meta.order }.sortedByDescending { it.modified }
        return pinned + unordered + ordered
    }

    private fun readHead(f: File): String = try {
        f.bufferedReader().use { r ->
            val buf = CharArray(700)
            val n = r.read(buf)
            if (n <= 0) "" else String(buf, 0, n)
        }
    } catch (e: Exception) {
        ""
    }

    /** Persist the visible order of a folder after a drag. */
    fun saveOrder(dir: File, entries: List<Entry>) {
        val meta = readMeta(dir)
        meta.pinned.clear()
        meta.pinned += entries.filter { it.pinned }.map { it.file.name }
        meta.order.clear()
        meta.order += entries.filter { !it.pinned }.map { it.file.name }
        writeMeta(dir, meta)
    }

    fun setPinned(file: File, pinned: Boolean) {
        val dir = file.parentFile ?: return
        val meta = readMeta(dir)
        meta.pinned.remove(file.name)
        if (pinned) meta.pinned.add(file.name)
        writeMeta(dir, meta)
    }

    fun isPinned(file: File): Boolean = file.parentFile?.let { file.name in readMeta(it).pinned } ?: false

    fun sanitize(name: String): String =
        name.trim().replace(Regex("[\\\\/:*?\"<>|\\n\\r\\t]"), "-").trimStart('.').ifBlank { "Untitled" }

    fun uniqueFile(dir: File, base: String, ext: String): File {
        var f = File(dir, base + ext)
        var i = 2
        while (f.exists()) {
            f = File(dir, "$base $i$ext")
            i++
        }
        return f
    }

    fun createNote(dir: File, name: String): File {
        val f = uniqueFile(dir, sanitize(name).removeMdExtension(), ".md")
        f.writeText("")
        return f
    }

    fun createFolder(dir: File, name: String): File {
        val f = uniqueFile(dir, sanitize(name), "")
        f.mkdirs()
        return f
    }

    private fun replaceInMeta(dir: File, old: String, new: String?) {
        val meta = readMeta(dir)
        val p = meta.pinned.indexOf(old)
        val o = meta.order.indexOf(old)
        if (p < 0 && o < 0) return
        if (p >= 0) if (new == null) meta.pinned.removeAt(p) else meta.pinned[p] = new
        if (o >= 0) if (new == null) meta.order.removeAt(o) else meta.order[o] = new
        writeMeta(dir, meta)
    }

    fun rename(file: File, newName: String): File? {
        val dir = file.parentFile ?: return null
        val clean = sanitize(newName)
        val target = if (file.isDirectory) File(dir, clean) else File(dir, clean.removeMdExtension() + ".md")
        if (target == file) return file
        if (target.exists()) return null
        if (!file.renameTo(target)) return null
        replaceInMeta(dir, file.name, target.name)
        return target
    }

    fun move(file: File, destDir: File): File? {
        val src = file.parentFile ?: return null
        if (src == destDir) return file
        if (file.isDirectory && destDir.absolutePath.startsWith(file.absolutePath + "/")) return null
        val target = if (file.isDirectory) uniqueFile(destDir, file.name, "")
        else uniqueFile(destDir, file.name.removeMdExtension(), ".md")
        if (!file.renameTo(target)) {
            // renameTo fails across mount points; fall back to copy + delete.
            try {
                file.copyRecursively(target)
                file.deleteRecursively()
            } catch (e: Exception) {
                return null
            }
        }
        replaceInMeta(src, file.name, null)
        return target
    }

    fun delete(file: File): Boolean {
        val dir = file.parentFile
        val ok = if (file.isDirectory) file.deleteRecursively() else file.delete()
        if (ok && dir != null) replaceInMeta(dir, file.name, null)
        return ok
    }

    /** Every visible folder below root, depth-first, for the Move dialog. */
    fun allFolders(root: File): List<File> {
        val out = mutableListOf(root)
        fun walk(d: File) {
            d.listFiles()?.filter { it.isVisibleFolder() }?.sortedBy { it.name.lowercase() }?.forEach {
                out += it
                walk(it)
            }
        }
        walk(root)
        return out
    }

    data class Hit(val file: File, val title: String, val folder: String, val snippet: String, val matchStart: Int, val matchLen: Int)

    fun search(root: File, query: String, limit: Int = 200): List<Hit> {
        val q = query.trim()
        if (q.isEmpty()) return emptyList()
        val titleHits = mutableListOf<Hit>()
        val bodyHits = mutableListOf<Hit>()
        root.walkTopDown()
            .onEnter { it == root || !it.name.startsWith(".") }
            .filter { it.isNote() }
            .forEach { f ->
                if (titleHits.size + bodyHits.size >= limit) return@forEach
                val folder = f.parentFile?.relativeTo(root)?.path ?: ""
                val title = f.name.removeMdExtension()
                val text = try { f.readText() } catch (e: Exception) { "" }
                val idx = text.indexOf(q, ignoreCase = true)
                if (title.contains(q, ignoreCase = true)) {
                    titleHits += Hit(f, title, folder, snippetAround(text, idx, q.length).first, -1, 0)
                } else if (idx >= 0) {
                    val (snip, at) = snippetAround(text, idx, q.length)
                    bodyHits += Hit(f, title, folder, snip, at, q.length)
                }
            }
        return titleHits + bodyHits
    }

    private fun snippetAround(text: String, idx: Int, len: Int): Pair<String, Int> {
        if (idx < 0) {
            val s = text.lineSequence().map { it.trim() }.firstOrNull { it.isNotEmpty() && !it.startsWith("#") } ?: ""
            return s.take(120) to -1
        }
        val start = maxOf(0, idx - 40)
        val end = minOf(text.length, idx + len + 80)
        val raw = text.substring(start, end)
        val prefix = if (start > 0) "…" else ""
        val flat = raw.replace('\n', ' ')
        return (prefix + flat) to (idx - start + prefix.length)
    }
}
