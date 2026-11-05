package app.noted.ui

import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.ui.platform.LocalContext
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.ui.unit.IntOffset
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.staggeredgrid.LazyStaggeredGridState
import androidx.compose.foundation.lazy.staggeredgrid.LazyVerticalStaggeredGrid
import androidx.compose.foundation.lazy.staggeredgrid.StaggeredGridCells
import androidx.compose.foundation.lazy.staggeredgrid.StaggeredGridItemSpan
import androidx.compose.foundation.lazy.staggeredgrid.rememberLazyStaggeredGridState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import app.noted.AppViewModel
import app.noted.Screen
import app.noted.data.Entry
import app.noted.data.Store
import kotlinx.coroutines.withTimeoutOrNull
import java.io.File

@Stable
private class DragState {
    var key by mutableStateOf<String?>(null)
    var pointer by mutableStateOf(Offset.Zero)
    var grab = Offset.Zero
    var lastSwap: String? = null
    var pressed by mutableStateOf<String?>(null)
}

private const val PINNED_HEADER = "::pinned"
private const val DIVIDER = "::divider"

@Composable
fun FolderScreen(vm: AppViewModel, dir: File) {
    LaunchedEffect(dir) { vm.refresh(dir) }
    val root = vm.root ?: dir
    val isRoot = dir == root
    val list = vm.entries[dir.absolutePath]
    var selected by remember { mutableStateOf(setOf<String>()) }
    val selecting = selected.isNotEmpty()
    var menu by remember { mutableStateOf(false) }
    var addMenu by remember { mutableStateOf(false) }
    var dialog by remember { mutableStateOf<String?>(null) }
    val selEntries = list.orEmpty().filter { it.key in selected }

    BackHandler(enabled = selecting) { selected = emptySet() }

    val ctx = LocalContext.current
    val importer = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) vm.importFiles(dir, uris) { ok, skipped ->
            val msg = buildString {
                append(if (ok == 1) "Imported 1 note" else "Imported $ok notes")
                if (skipped > 0) append(" · skipped $skipped (not .md)")
            }
            Toast.makeText(ctx, msg, Toast.LENGTH_SHORT).show()
        }
    }

    Box(Modifier.fillMaxSize().background(C.Black)) {
        Column(Modifier.fillMaxSize().statusBarsPadding()) {
            // Header
            if (selecting) {
                Row(
                    Modifier.fillMaxWidth().padding(start = 20.dp, end = 12.dp, top = 12.dp, bottom = 8.dp).height(44.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        if (selected.size == 1) "Drag to reorder" else "${selected.size} selected",
                        fontSize = 13.sp, color = C.Text3, modifier = Modifier.weight(1f),
                    )
                    PillButton("Done", primary = true) { selected = emptySet() }
                }
            } else {
                Row(
                    Modifier.fillMaxWidth().padding(start = if (isRoot) 20.dp else 6.dp, end = 12.dp, top = 12.dp, bottom = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    if (!isRoot) IconBtn(Icons.Back, "Back") { vm.pop() }
                    Text(
                        if (isRoot) "Noted" else dir.name,
                        fontSize = if (isRoot) 28.sp else 21.sp, fontWeight = FontWeight.ExtraBold,
                        letterSpacing = (-0.5).sp, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    IconBtn(Icons.Search, "Search all notes") { vm.push(Screen.Search) }
                    Box {
                        IconBtn(Icons.More, "More") { menu = true }
                        Menu(
                            menu, { menu = false },
                            listOf(
                                "Refresh" to { vm.refresh(dir) },
                                "Change notes folder" to { vm.changeRoot() },
                            ),
                        )
                    }
                }
            }

            if (list == null) return@Column
            if (list.isEmpty()) {
                Box(Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
                    Text(
                        "Nothing here yet.\nTap + to write a note.", color = C.Muted, fontSize = 13.sp,
                        lineHeight = 21.sp, textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                    )
                }
            } else {
                Grid(
                    entries = list,
                    selected = selected,
                    onTap = { e ->
                        if (selecting) {
                            selected = if (e.key in selected) selected - e.key else selected + e.key
                        } else if (e.isFolder) vm.push(Screen.Folder(e.file))
                        else vm.push(Screen.Note(e.file))
                    },
                    onLongPress = { e -> selected = selected + e.key },
                    onSwap = { a, b -> vm.moveEntry(dir, a, b) },
                    onDrop = { vm.persistOrder(dir) },
                )
            }
        }

        AnimatedVisibility(
            visible = selecting,
            enter = slideInVertically(spring(stiffness = 500f)) { it } + fadeIn(),
            exit = slideOutVertically(tween(180)) { it } + fadeOut(tween(180)),
            modifier = Modifier.align(Alignment.BottomCenter),
        ) {
            SelectionBar(
                allPinned = selEntries.isNotEmpty() && selEntries.all { it.pinned },
                single = selEntries.size == 1,
                onPin = { pin -> vm.setPinned(selEntries.map { it.file }, pin); selected = emptySet() },
                onMove = { dialog = "move" },
                onRename = { dialog = "rename" },
                onDelete = { dialog = "delete" },
                modifier = Modifier,
            )
        }
        AnimatedVisibility(
            visible = !selecting,
            enter = scaleIn(spring(stiffness = 500f, dampingRatio = 0.7f), initialScale = 0.6f) + fadeIn(),
            exit = scaleOut(tween(150), targetScale = 0.6f) + fadeOut(tween(150)),
            modifier = Modifier.align(Alignment.BottomEnd),
        ) {
            Box(Modifier.navigationBarsPadding().padding(end = 20.dp, bottom = 24.dp)) {
                Box(
                    Modifier.size(60.dp).clip(CircleShape).background(C.White).clickable(onClickLabel = "New note or folder") { addMenu = true },
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Plus, "New note or folder", tint = C.Black, modifier = Modifier.size(26.dp)) }
                Menu(
                    addMenu, { addMenu = false },
                    listOf(
                        "New note" to { dialog = "newNote" },
                        "New folder" to { dialog = "newFolder" },
                        "Import .md files" to { importer.launch(arrayOf("text/*", "application/octet-stream")) },
                    ),
                )
            }
        }
    }

    when (dialog) {
        "newNote" -> NameDialog("New note", "", "Create", { dialog = null }) { dialog = null; vm.newNote(dir, it) }
        "newFolder" -> NameDialog("New folder", "", "Create", { dialog = null }) { dialog = null; vm.newFolder(dir, it) }
        "rename" -> selEntries.singleOrNull()?.let { e ->
            NameDialog("Rename", e.title, "Rename", { dialog = null }) { name ->
                dialog = null; selected = emptySet(); vm.rename(e.file, name)
            }
        } ?: run { dialog = null }
        "delete" -> {
            val folders = selEntries.count { it.isFolder }
            val body = when {
                selEntries.size == 1 && folders == 1 -> "\"${selEntries[0].title}\" and everything inside it will be deleted from your phone."
                selEntries.size == 1 -> "\"${selEntries[0].title}\" will be deleted from your phone."
                else -> "${selEntries.size} items will be deleted from your phone" + (if (folders > 0) ", including everything inside the folders." else ".")
            }
            ConfirmDialog("Delete?", body, "Delete", { dialog = null }) {
                dialog = null; vm.delete(selEntries.map { it.file }); selected = emptySet()
            }
        }
        "move" -> {
            val folders = remember(root) { Store.allFolders(root) }
            MoveDialog(root, folders, selEntries.map { it.file }, { dialog = null }) { dest ->
                dialog = null; vm.move(selEntries.map { it.file }, dest); selected = emptySet()
            }
        }
    }
}

@Composable
private fun Grid(
    entries: List<Entry>,
    selected: Set<String>,
    onTap: (Entry) -> Unit,
    onLongPress: (Entry) -> Unit,
    onSwap: (String, String) -> Unit,
    onDrop: () -> Unit,
) {
    val state = rememberLazyStaggeredGridState()
    val drag = remember { DragState() }
    val haptic = LocalHapticFeedback.current
    val edge = with(LocalDensity.current) { 72.dp.toPx() }
    val byKey = entries.associateBy { it.key }
    val currentByKey by rememberUpdatedState(byKey)
    val tap by rememberUpdatedState(onTap)
    val long by rememberUpdatedState(onLongPress)
    val swap by rememberUpdatedState(onSwap)
    val drop by rememberUpdatedState(onDrop)

    val pinned = entries.filter { it.pinned }
    val rest = entries.filter { !it.pinned }

    LazyVerticalStaggeredGrid(
        columns = StaggeredGridCells.Fixed(2),
        state = state,
        verticalItemSpacing = 8.dp,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        contentPadding = PaddingValues(top = 4.dp, bottom = 140.dp),
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 12.dp)
            .pointerInput(Unit) {
                fun itemAt(p: Offset) = state.layoutInfo.visibleItemsInfo.firstOrNull {
                    val y = it.offset.y - state.layoutInfo.viewportStartOffset
                    p.x >= it.offset.x && p.x < it.offset.x + it.size.width && p.y >= y && p.y < y + it.size.height
                }

                fun cardAt(p: Offset): Entry? = (itemAt(p)?.key as? String)?.let { currentByKey[it] }

                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    drag.pressed = cardAt(down.position)?.key
                    var outcome = 0 // 0 = scroll/cancel, 1 = tap, 2 = long press
                    val res = withTimeoutOrNull(viewConfiguration.longPressTimeoutMillis) {
                        while (true) {
                            val ev = awaitPointerEvent()
                            val c = ev.changes.firstOrNull { it.id == down.id }
                            if (c == null) { outcome = 0; break }
                            if (!c.pressed) { outcome = if (c.isConsumed) 0 else 1; break }
                            if (c.isConsumed || (c.position - down.position).getDistance() > viewConfiguration.touchSlop) {
                                outcome = 0; break
                            }
                        }
                    }
                    if (res == null) outcome = 2
                    if (outcome != 1) drag.pressed = null
                    if (outcome == 1) {
                        val hit = cardAt(down.position)
                        drag.pressed = null
                        hit?.let { tap(it) }
                        return@awaitEachGesture
                    }
                    if (outcome != 2) return@awaitEachGesture
                    val info = itemAt(down.position) ?: return@awaitEachGesture
                    val entry = (info.key as? String)?.let { currentByKey[it] } ?: return@awaitEachGesture
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    long(entry)
                    drag.key = entry.key
                    drag.grab = down.position - Offset(info.offset.x.toFloat(), (info.offset.y - state.layoutInfo.viewportStartOffset).toFloat())
                    drag.pointer = down.position
                    drag.lastSwap = null

                    fun step() {
                        val p = drag.pointer
                        if (p.y < edge) state.dispatchRawDelta(-(edge - p.y) / 5f)
                        else if (p.y > size.height - edge) state.dispatchRawDelta((p.y - (size.height - edge)) / 5f)
                        val target = cardAt(p)
                        val me = currentByKey[entry.key]
                        when {
                            target == null || target.key == entry.key -> drag.lastSwap = null
                            target.key == drag.lastSwap -> Unit
                            me != null && target.pinned == me.pinned -> {
                                swap(entry.key, target.key)
                                drag.lastSwap = target.key
                            }
                        }
                    }

                    while (true) {
                        val ev = withTimeoutOrNull(16) { awaitPointerEvent(PointerEventPass.Initial) }
                        if (ev == null) { step(); continue }
                        val c = ev.changes.firstOrNull { it.id == down.id } ?: break
                        c.consume()
                        if (!c.pressed) break
                        drag.pointer = c.position
                        step()
                    }
                    drag.key = null
                    drop()
                }
            },
    ) {
        if (pinned.isNotEmpty()) {
            item(key = PINNED_HEADER, span = StaggeredGridItemSpan.FullLine) {
                Text(
                    "PINNED", fontSize = 11.sp, fontWeight = FontWeight.Bold, letterSpacing = 1.sp, color = C.Muted,
                    modifier = Modifier.padding(start = 6.dp, top = 4.dp),
                )
            }
            items(pinned.size, key = { pinned[it].key }) { i ->
                val e = pinned[i]
                DraggableCard(e, e.key in selected, drag, state, Modifier.animateItem(fadeInSpec = tween(220), fadeOutSpec = tween(160), placementSpec = if (drag.key == e.key) null else androidx.compose.animation.core.spring(stiffness = 600f, visibilityThreshold = IntOffset.VisibilityThreshold)))
            }
            item(key = DIVIDER, span = StaggeredGridItemSpan.FullLine) {
                Box(Modifier.padding(horizontal = 4.dp, vertical = 6.dp).fillMaxWidth().height(2.dp).background(C.Divider))
            }
        }
        items(rest.size, key = { rest[it].key }) { i ->
            val e = rest[i]
            DraggableCard(e, e.key in selected, drag, state, Modifier.animateItem(fadeInSpec = tween(220), fadeOutSpec = tween(160), placementSpec = if (drag.key == e.key) null else androidx.compose.animation.core.spring(stiffness = 600f, visibilityThreshold = IntOffset.VisibilityThreshold)))
        }
    }
}

@Composable
private fun DraggableCard(e: Entry, selected: Boolean, drag: DragState, state: LazyStaggeredGridState, modifier: Modifier) {
    val dragging = drag.key == e.key
    val press by animateFloatAsState(if (drag.pressed == e.key && !dragging) 0.96f else 1f, spring(stiffness = 900f), label = "press")
    val lift by animateFloatAsState(if (dragging) 1f else 0f, spring(stiffness = 600f), label = "lift")
    Box(
        modifier
            .zIndex(if (dragging) 1f else 0f)
            .drawBehind {
                if (dragging) {
                    val r = 22.dp.toPx()
                    drawRoundRect(
                        color = Color(0xFF6A6A6A), cornerRadius = CornerRadius(r),
                        style = Stroke(width = 2.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f))),
                    )
                }
            },
    ) {
        Card(
            e, selected || dragging,
            Modifier.graphicsLayer {
                if (dragging) {
                    val info = state.layoutInfo.visibleItemsInfo.firstOrNull { it.key == e.key }
                    if (info != null) {
                        translationX = drag.pointer.x - drag.grab.x - info.offset.x
                        translationY = drag.pointer.y - drag.grab.y - (info.offset.y - state.layoutInfo.viewportStartOffset)
                    }
                }
                scaleX = press * (1f + 0.05f * lift); scaleY = scaleX
                rotationZ = -3f * lift
            },
            raised = dragging,
        )
    }
}

@Composable
private fun Card(e: Entry, highlighted: Boolean, modifier: Modifier, raised: Boolean = false) {
    val shape = RoundedCornerShape(22.dp)
    Column(
        modifier
            .fillMaxWidth()
            .clip(shape)
            .background(if (raised) Color(0xFF111111) else C.Black)
            .border(if (highlighted) 2.5.dp else LineW, if (highlighted) C.White else C.Border, shape)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(if (e.isFolder) 8.dp else 6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (e.isFolder) Icon(Icons.Folder, null, tint = C.White, modifier = Modifier.size(16.dp))
            Text(
                e.title, fontSize = 15.sp, fontWeight = FontWeight.ExtraBold, lineHeight = 20.sp,
                maxLines = 3, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f),
            )
            if (e.pinned) Icon(Icons.PinFilled, "Pinned", tint = C.White, modifier = Modifier.size(14.dp))
        }
        if (e.isFolder) {
            if (e.children.isNotEmpty()) {
                Text(
                    e.children.joinToString("\n"), fontSize = 12.sp, lineHeight = 20.sp, color = C.Text3,
                    maxLines = 3, overflow = TextOverflow.Ellipsis,
                )
            }
            Text(if (e.childCount == 1) "1 item" else "${e.childCount} items", fontSize = 11.sp, color = C.Muted)
        } else {
            NotePreview(e.preview, e.title)
        }
    }
}

/** A light rendering of a note's first lines for its card. */
@Composable
private fun NotePreview(src: String, title: String) {
    val lines = remember(src) {
        val all = src.split('\n').map { it.trimEnd() }.dropWhile { it.isBlank() }.toMutableList()
        if (all.isNotEmpty() && all[0].startsWith("# ")) all.removeAt(0)
        // Drop YAML front matter.
        if (all.firstOrNull() == "---") {
            val end = all.drop(1).indexOf("---")
            if (end >= 0) repeat(end + 2) { all.removeAt(0) }
        }
        all.filter { it.isNotBlank() && !it.trimStart().startsWith("```") }.take(8)
    }
    if (lines.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        for (l in lines) {
            val info = classifyLine(l)
            when (info.kind) {
                LineKind.TASK -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Box(
                        Modifier.padding(start = (indentWidth(l, info.indent) * 4).dp).size(11.dp).clip(RoundedCornerShape(3.dp))
                            .then(if (info.checked) Modifier.background(C.White) else Modifier.border(2.dp, C.Muted, RoundedCornerShape(3.dp))),
                    )
                    Text(
                        renderInline(l.substring(info.prefixEnd)), fontSize = 12.sp, lineHeight = 18.sp,
                        color = if (info.checked) C.Muted else C.Text3, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        textDecoration = if (info.checked) TextDecoration.LineThrough else null,
                    )
                }
                LineKind.HEADING -> Text(
                    renderInline(l.substring(info.prefixEnd)), fontSize = 12.sp, lineHeight = 18.sp, fontWeight = FontWeight.Bold,
                    color = C.White, maxLines = 2, overflow = TextOverflow.Ellipsis,
                )
                LineKind.RULE -> Box(Modifier.padding(vertical = 4.dp).fillMaxWidth().height(2.dp).background(C.Border))
                else -> Text(
                    renderInline(l.trim()), fontSize = 12.sp, lineHeight = 18.sp, color = C.Text3,
                    maxLines = 4, overflow = TextOverflow.Ellipsis,
                    modifier = if (info.kind == LineKind.BULLET || info.kind == LineKind.NUMBER)
                        Modifier.padding(start = (indentWidth(l, info.indent) * 4).dp) else Modifier,
                )
            }
        }
    }
}

@Composable
private fun SelectionBar(
    allPinned: Boolean,
    single: Boolean,
    onPin: (Boolean) -> Unit,
    onMove: () -> Unit,
    onRename: () -> Unit,
    onDelete: () -> Unit,
    modifier: Modifier,
) {
    Row(
        modifier.navigationBarsPadding().padding(start = 12.dp, end = 12.dp, bottom = 20.dp).fillMaxWidth().height(64.dp)
            .clip(RoundedCornerShape(32.dp)).background(C.Pill).padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        BarAction(if (allPinned) Icons.PinFilled else Icons.Pin, if (allPinned) "Unpin" else "Pin") { onPin(!allPinned) }
        BarAction(Icons.Move, "Move", onClick = onMove)
        if (single) BarAction(Icons.Rename, "Rename", onClick = onRename)
        BarAction(Icons.Delete, "Delete", onClick = onDelete)
    }
}

@Composable
private fun androidx.compose.foundation.layout.RowScope.BarAction(icon: androidx.compose.ui.graphics.vector.ImageVector, label: String, onClick: () -> Unit) {
    Column(
        Modifier.weight(1f).height(48.dp).clip(RoundedCornerShape(24.dp)).clickable(onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center,
    ) {
        Icon(icon, null, tint = C.Text2, modifier = Modifier.size(18.dp))
        Spacer(Modifier.height(2.dp))
        Text(label, fontSize = 11.sp, color = C.Text2)
    }
}
