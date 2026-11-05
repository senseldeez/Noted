package app.noted.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.noted.AppViewModel
import app.noted.Screen
import app.noted.data.Store
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

@Composable
fun SearchScreen(vm: AppViewModel) {
    val root = vm.root ?: return
    var query by rememberSaveable { mutableStateOf("") }
    var hits by remember { mutableStateOf<List<Store.Hit>>(emptyList()) }
    var searching by remember { mutableStateOf(false) }
    val fr = remember { FocusRequester() }
    LaunchedEffect(Unit) { try { fr.requestFocus() } catch (_: Exception) {} }
    LaunchedEffect(query) {
        if (query.isBlank()) { hits = emptyList(); return@LaunchedEffect }
        delay(180)
        searching = true
        hits = withContext(Dispatchers.IO) { Store.search(root, query) }
        searching = false
    }

    Column(Modifier.fillMaxSize().background(C.Black).statusBarsPadding().imePadding()) {
        Row(Modifier.fillMaxWidth().padding(start = 6.dp, end = 16.dp, top = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
            IconBtn(Icons.Back, "Back") { vm.pop() }
            BasicTextField(
                value = query, onValueChange = { query = it }, singleLine = true,
                textStyle = TextStyle(fontFamily = Sans, fontSize = 15.sp, color = C.White),
                cursorBrush = SolidColor(C.White),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                modifier = Modifier.weight(1f).focusRequester(fr),
                decorationBox = { inner ->
                    Box(
                        Modifier.fillMaxWidth().height(48.dp).border(LineW, C.Border, RoundedCornerShape(24.dp)).padding(horizontal = 18.dp),
                        contentAlignment = Alignment.CenterStart,
                    ) {
                        if (query.isEmpty()) Text("Search all notes", fontSize = 15.sp, color = C.Muted)
                        inner()
                    }
                },
            )
        }
        if (query.isNotBlank() && hits.isEmpty() && !searching) {
            Text("No matches.", fontSize = 13.sp, color = C.Muted, modifier = Modifier.padding(24.dp))
        }
        LazyColumn(
            contentPadding = PaddingValues(start = 12.dp, end = 12.dp, bottom = 40.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(hits, key = { it.file.absolutePath }) { h ->
                Column(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).border(LineW, C.Border, RoundedCornerShape(20.dp))
                        .clickable { vm.push(Screen.Note(h.file)) }.padding(14.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    Text(h.title, fontSize = 14.sp, fontWeight = FontWeight.Bold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (h.folder.isNotEmpty()) Text(h.folder + "/", fontSize = 11.sp, color = C.Muted, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    if (h.snippet.isNotBlank()) {
                        val s = h.snippet
                        val text = buildAnnotatedString {
                            if (h.matchStart in s.indices) {
                                val end = (h.matchStart + h.matchLen).coerceAtMost(s.length)
                                append(s.substring(0, h.matchStart))
                                withStyle(androidx.compose.ui.text.SpanStyle(color = C.Black, background = C.White, fontWeight = FontWeight.Bold)) {
                                    append(s.substring(h.matchStart, end))
                                }
                                append(s.substring(end))
                            } else append(s)
                        }
                        Text(text, fontSize = 12.sp, lineHeight = 18.sp, color = C.Text3, maxLines = 3, overflow = TextOverflow.Ellipsis)
                    }
                }
            }
        }
    }
}
