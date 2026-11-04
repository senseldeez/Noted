package app.noted.ui

import android.text.format.DateUtils
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.noted.AppViewModel
import app.noted.Screen
import app.noted.data.Store
import app.noted.data.removeMdExtension
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

private fun readSafe(f: File): String = try { f.readText() } catch (e: Exception) { "" }
private fun writeSafe(f: File, s: String) = try { f.writeText(s); true } catch (e: Exception) { false }

private fun startsWithH1(src: String): Boolean =
    src.lineSequence().firstOrNull { it.isNotBlank() }?.let { classifyLine(it).let { i -> i.kind == LineKind.HEADING && i.level == 1 } } == true

@Composable
fun NoteScreen(vm: AppViewModel, screen: Screen.Note) {
    if (screen.editing) EditScreen(vm, screen.file) else ReadScreen(vm, screen.file)
}

@Composable
private fun ReadScreen(vm: AppViewModel, file: File) {
    var text by remember(file) { mutableStateOf<String?>(null) }
    var pinned by remember(file) { mutableStateOf(false) }
    var modified by remember(file) { mutableStateOf(0L) }
    var menu by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    LaunchedEffect(file) {
        withContext(Dispatchers.IO) {
            val t = readSafe(file)
            val p = Store.isPinned(file)
            val m = file.lastModified()
            withContext(Dispatchers.Main) { text = t; pinned = p; modified = m }
        }
    }
    val root = vm.root
    val folderName = file.parentFile?.let { if (it == root) "Noted" else it.name } ?: ""

    Box(Modifier.fillMaxSize().background(C.Black)) {
        Column(Modifier.fillMaxSize().statusBarsPadding()) {
            Row(Modifier.fillMaxWidth().padding(start = 6.dp, end = 12.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                IconBtn(Icons.Back, "Back") { vm.pop() }
                Text(folderName, fontSize = 12.sp, color = C.Muted, modifier = Modifier.weight(1f), maxLines = 1, overflow = TextOverflow.Ellipsis)
                IconBtn(if (pinned) Icons.PinFilled else Icons.Pin, if (pinned) "Unpin note" else "Pin note", size = 18.dp) {
                    pinned = !pinned
                    vm.setPinned(listOf(file), pinned)
                }
                Box {
                    IconBtn(Icons.More, "Note options") { menu = true }
                    Menu(menu, { menu = false }, listOf("Rename" to { dialog = "rename" }, "Delete" to { dialog = "delete" }))
                }
            }
            val t = text ?: return@Column
            Column(
                Modifier.fillMaxSize().verticalScroll(rememberScrollState())
                    .padding(start = 22.dp, end = 22.dp, top = 8.dp, bottom = 120.dp),
            ) {
                if (!startsWithH1(t)) {
                    Text(
                        file.name.removeMdExtension(), fontSize = 24.sp, lineHeight = 30.sp,
                        fontWeight = FontWeight.Bold, letterSpacing = (-0.4).sp,
                        modifier = Modifier.padding(bottom = 14.dp),
                    )
                }
                if (t.isBlank()) {
                    Text("Empty note. Tap the pencil to start writing.", fontSize = 13.sp, color = C.Muted)
                } else {
                    MarkdownReader(t, onToggleTask = { line ->
                        val nt = toggleTaskLine(t, line)
                        text = nt
                        scope.launch(Dispatchers.IO) { writeSafe(file, nt); modified = file.lastModified() }
                    })
                }
                Spacer(Modifier.height(18.dp))
                Box(Modifier.fillMaxWidth().height(1.dp).background(C.Divider))
                val words = wordCount(t)
                val ago = if (modified > 0) DateUtils.getRelativeTimeSpanString(modified, System.currentTimeMillis(), DateUtils.MINUTE_IN_MILLIS).toString().lowercase() else ""
                Text(
                    "$words ${if (words == 1) "word" else "words"}" + (if (ago.isNotEmpty()) " · edited $ago" else ""),
                    fontSize = 11.sp, color = C.Muted, modifier = Modifier.padding(top = 10.dp),
                )
            }
        }
        Box(
            Modifier.align(Alignment.BottomEnd).navigationBarsPadding().padding(end = 20.dp, bottom = 24.dp)
                .size(60.dp).clip(CircleShape).background(C.White)
                .clickable(onClickLabel = "Start editing") { vm.replaceTop(Screen.Note(file, editing = true)) },
            contentAlignment = Alignment.Center,
        ) { Icon(Icons.Edit, "Start editing", tint = C.Black, modifier = Modifier.size(24.dp)) }
    }

    when (dialog) {
        "rename" -> NameDialog("Rename", file.name.removeMdExtension(), "Rename", { dialog = null }) { name ->
            dialog = null
            vm.rename(file, name)
        }
        "delete" -> ConfirmDialog("Delete?", "\"${file.name.removeMdExtension()}\" will be deleted from your phone.", "Delete", { dialog = null }) {
            dialog = null
            vm.delete(listOf(file))
            vm.pop()
        }
    }
}

@Composable
private fun EditScreen(vm: AppViewModel, file: File) {
    var value by remember(file) { mutableStateOf<TextFieldValue?>(null) }
    var saved by remember(file) { mutableStateOf("") }
    var history by remember(file) { mutableStateOf<History?>(null) }
    val focus = remember { FocusRequester() }
    val scope = rememberCoroutineScope()

    LaunchedEffect(file) {
        val t = withContext(Dispatchers.IO) { readSafe(file) }
        val v = TextFieldValue(t, TextRange(t.length))
        saved = t
        history = History(v)
        value = v
        delay(50)
        try { focus.requestFocus() } catch (_: Exception) {}
    }

    val v = value
    // Autosave shortly after typing stops.
    LaunchedEffect(v?.text) {
        val t = v?.text ?: return@LaunchedEffect
        if (t == saved) return@LaunchedEffect
        delay(600)
        withContext(Dispatchers.IO) { writeSafe(file, t) }
        saved = t
    }
    val latest by rememberUpdatedState(value?.text)
    DisposableEffect(file) {
        onDispose {
            val t = latest
            if (t != null && t != saved) Thread { writeSafe(file, t) }.start()
        }
    }

    fun done() {
        val t = value?.text
        scope.launch {
            if (t != null && t != saved) withContext(Dispatchers.IO) { writeSafe(file, t) }
            if (t != null) saved = t
            vm.replaceTop(Screen.Note(file, editing = false))
        }
    }
    BackHandler { done() }

    fun change(next: TextFieldValue, force: Boolean = false) {
        val prev = value ?: return
        val fixed = if (force) next else Ops.afterChange(prev, next)
        history?.record(prev, fixed, force)
        value = fixed
    }

    Column(Modifier.fillMaxSize().background(C.Black).statusBarsPadding().navigationBarsPadding().imePadding()) {
        val h = history
        h?.version // read so undo/redo enablement recomposes
        Row(Modifier.fillMaxWidth().padding(start = 6.dp, end = 12.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconBtn(Icons.Back, "Done editing") { done() }
            Text(
                if (v == null || v.text == saved) "Saved" else "Editing",
                fontSize = 12.sp, color = C.Muted, modifier = Modifier.weight(1f),
            )
            IconBtn(Icons.Undo, "Undo", tint = if (h?.canUndo == true) C.White else C.Muted, size = 20.dp) {
                val cur = value ?: return@IconBtn
                h?.undo(cur)?.let { value = it }
            }
            IconBtn(Icons.Redo, "Redo", tint = if (h?.canRedo == true) C.White else C.Muted, size = 20.dp) {
                val cur = value ?: return@IconBtn
                h?.redo(cur)?.let { value = it }
            }
        }
        if (v == null) return@Column
        Column(
            Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState())
                .padding(start = 22.dp, end = 22.dp, top = 8.dp, bottom = 24.dp),
        ) {
            if (!startsWithH1(v.text)) {
                Text(
                    file.name.removeMdExtension(), fontSize = 24.sp, lineHeight = 30.sp,
                    fontWeight = FontWeight.Bold, letterSpacing = (-0.4).sp, color = C.White,
                    modifier = Modifier.padding(bottom = 14.dp),
                )
            }
            LiveEditor(v, { change(it) }, focus, Modifier.fillMaxWidth())
            // Tapping below the text focuses the end of the note.
            Spacer(
                Modifier.fillMaxWidth().height(240.dp).clickable(indication = null, interactionSource = null) {
                    value = v.copy(selection = TextRange(v.text.length))
                    try { focus.requestFocus() } catch (_: Exception) {}
                },
            )
        }
        FormatToolbar(v, { change(it, force = true) }, Modifier.padding(bottom = 8.dp))
    }
}
