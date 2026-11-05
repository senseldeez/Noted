package app.noted

import android.app.Application
import android.content.Context
import android.net.Uri
import android.os.Environment
import android.provider.OpenableColumns
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import app.noted.data.Entry
import app.noted.data.Store
import app.noted.data.removeMdExtension
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

sealed class Screen {
    data object Setup : Screen()
    data class Folder(val dir: File) : Screen()
    data class Note(val file: File, val editing: Boolean = false) : Screen()
    data object Search : Screen()
}

class AppViewModel(app: Application) : AndroidViewModel(app) {
    private val prefs = app.getSharedPreferences("noted", Context.MODE_PRIVATE)

    var root by mutableStateOf(prefs.getString("root", null)?.let(::File))
        private set
    var hasAccess by mutableStateOf(Environment.isExternalStorageManager())
        private set

    val stack = mutableStateListOf<Screen>()
    val entries = mutableStateMapOf<String, List<Entry>>()

    /** Folded heading sections per note (by heading text), shared by reading and editing mode. */
    private val folds = mutableStateMapOf<String, Set<String>>()

    fun foldsFor(file: File): Set<String> = folds[file.absolutePath] ?: emptySet()

    fun toggleFold(file: File, key: String) {
        val cur = foldsFor(file)
        folds[file.absolutePath] = if (key in cur) cur - key else cur + key
    }

    init {
        resetStack()
    }

    private fun ready() = hasAccess && root?.let { it.isDirectory || it.mkdirs() } == true

    private fun resetStack() {
        stack.clear()
        stack += if (ready()) Screen.Folder(root!!) else Screen.Setup
    }

    fun onResume() {
        val had = hasAccess
        hasAccess = Environment.isExternalStorageManager()
        if (stack.lastOrNull() == Screen.Setup && ready()) resetStack()
        else if (had && !hasAccess) resetStack()
        stack.filterIsInstance<Screen.Folder>().forEach { refresh(it.dir) }
    }

    fun chooseRoot(dir: File) {
        dir.mkdirs()
        root = dir
        prefs.edit().putString("root", dir.absolutePath).apply()
        resetStack()
    }

    fun changeRoot() {
        stack.clear()
        stack += Screen.Setup
    }

    fun push(s: Screen) {
        stack += s
    }

    fun replaceTop(s: Screen) {
        if (stack.isNotEmpty()) stack[stack.size - 1] = s else stack += s
    }

    /** Returns false when there's nothing to pop (let the system finish the activity). */
    fun pop(): Boolean {
        if (stack.size <= 1) return false
        stack.removeAt(stack.size - 1)
        (stack.last() as? Screen.Folder)?.let { refresh(it.dir) }
        return true
    }

    fun refresh(dir: File) {
        viewModelScope.launch {
            val list = withContext(Dispatchers.IO) { Store.list(dir) }
            entries[dir.absolutePath] = list
        }
    }

    /* ---- Grid reorder (in memory while dragging, persisted on drop) ---- */

    fun moveEntry(dir: File, fromKey: String, toKey: String) {
        val list = entries[dir.absolutePath]?.toMutableList() ?: return
        val from = list.indexOfFirst { it.key == fromKey }
        val to = list.indexOfFirst { it.key == toKey }
        if (from < 0 || to < 0 || list[from].pinned != list[to].pinned) return
        val item = list.removeAt(from)
        list.add(to, item)
        entries[dir.absolutePath] = list
    }

    fun persistOrder(dir: File) {
        val list = entries[dir.absolutePath] ?: return
        viewModelScope.launch(Dispatchers.IO) { Store.saveOrder(dir, list) }
    }

    /* ---- File operations ---- */

    private fun io(dir: File?, block: () -> Unit) {
        viewModelScope.launch {
            withContext(Dispatchers.IO) { block() }
            dir?.let { refresh(it) }
        }
    }

    fun setPinned(files: List<File>, pinned: Boolean) {
        val dir = files.firstOrNull()?.parentFile
        io(dir) {
            files.forEach { Store.setPinned(it, pinned) }
        }
    }

    fun delete(files: List<File>) {
        val dir = files.firstOrNull()?.parentFile
        io(dir) { files.forEach { Store.delete(it) } }
    }

    fun move(files: List<File>, dest: File) {
        val dir = files.firstOrNull()?.parentFile
        io(dir) { files.forEach { Store.move(it, dest) } }
    }

    fun rename(file: File, name: String, onDone: (File?) -> Unit = {}) {
        viewModelScope.launch {
            val r = withContext(Dispatchers.IO) { Store.rename(file, name) }
            file.parentFile?.let { refresh(it) }
            if (r != null) {
                // Keep any open screen pointing at the renamed file.
                for (i in stack.indices) {
                    val s = stack[i]
                    if (s is Screen.Note && s.file == file) stack[i] = s.copy(file = r)
                }
            }
            onDone(r)
        }
    }

    fun newNote(dir: File, name: String) {
        viewModelScope.launch {
            val f = withContext(Dispatchers.IO) { Store.createNote(dir, name) }
            refresh(dir)
            push(Screen.Note(f, editing = true))
        }
    }

    /** Copy picked .md files into [dir]. Reports (imported, skipped). */
    fun importFiles(dir: File, uris: List<Uri>, onDone: (Int, Int) -> Unit) {
        val cr = getApplication<Application>().contentResolver
        viewModelScope.launch {
            var ok = 0
            var skipped = 0
            withContext(Dispatchers.IO) {
                for (uri in uris) {
                    val name = try {
                        cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                            if (c.moveToFirst()) c.getString(0) else null
                        }
                    } catch (e: Exception) { null } ?: uri.lastPathSegment?.substringAfterLast('/') ?: "Imported"
                    val mime = try { cr.getType(uri) } catch (e: Exception) { null } ?: ""
                    val isMd = name.endsWith(".md", true) || name.endsWith(".markdown", true) || mime.contains("markdown")
                    if (!isMd) { skipped++; continue }
                    try {
                        val target = Store.uniqueFile(dir, Store.sanitize(name).removeMdExtension(), ".md")
                        cr.openInputStream(uri)?.use { input -> target.outputStream().use { input.copyTo(it) } }
                            ?: throw IllegalStateException("unreadable")
                        ok++
                    } catch (e: Exception) {
                        skipped++
                    }
                }
            }
            refresh(dir)
            onDone(ok, skipped)
        }
    }

    fun newFolder(dir: File, name: String) {
        io(dir) { Store.createFolder(dir, name) }
    }
}
