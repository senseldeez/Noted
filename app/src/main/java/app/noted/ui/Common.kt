package app.noted.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.io.File

@Composable
fun IconBtn(
    icon: ImageVector,
    label: String,
    tint: Color = C.White,
    size: Dp = 22.dp,
    onClick: () -> Unit,
) {
    Box(
        Modifier.size(44.dp).clip(RoundedCornerShape(22.dp)).clickable(onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = label, tint = tint, modifier = Modifier.size(size))
    }
}

@Composable
fun PillButton(text: String, primary: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Box(
        modifier
            .heightIn(min = 44.dp)
            .clip(RoundedCornerShape(22.dp))
            .then(if (primary) Modifier.background(C.White) else Modifier.border(1.dp, C.Border, RoundedCornerShape(22.dp)))
            .clickable(onClick = onClick)
            .padding(horizontal = 18.dp, vertical = 11.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, fontSize = 13.sp, fontWeight = FontWeight.Bold, color = if (primary) C.Black else C.White)
    }
}

@Composable
fun Menu(expanded: Boolean, onDismiss: () -> Unit, items: List<Pair<String, () -> Unit>>) {
    DropdownMenu(
        expanded = expanded, onDismissRequest = onDismiss,
        containerColor = Color(0xFF141414), shape = RoundedCornerShape(16.dp),
        modifier = Modifier.border(1.dp, C.Border, RoundedCornerShape(16.dp)),
    ) {
        items.forEach { (label, action) ->
            DropdownMenuItem(
                text = { Text(label, fontSize = 14.sp) },
                onClick = { onDismiss(); action() },
            )
        }
    }
}

@Composable
fun NameDialog(title: String, initial: String, confirm: String, onDismiss: () -> Unit, onConfirm: (String) -> Unit) {
    var value by remember { mutableStateOf(TextFieldValue(initial, TextRange(0, initial.length))) }
    val fr = remember { FocusRequester() }
    LaunchedEffect(Unit) { fr.requestFocus() }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Color(0xFF111111),
        shape = RoundedCornerShape(24.dp),
        title = { Text(title, fontSize = 16.sp, fontWeight = FontWeight.Bold) },
        text = {
            BasicTextField(
                value = value, onValueChange = { value = it }, singleLine = true,
                textStyle = TextStyle(fontFamily = Mono, fontSize = 15.sp, color = C.White),
                cursorBrush = SolidColor(C.White),
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { if (value.text.isNotBlank()) onConfirm(value.text) }),
                modifier = Modifier.fillMaxWidth().focusRequester(fr),
                decorationBox = { inner ->
                    Box(
                        Modifier.fillMaxWidth().border(1.dp, C.Border, RoundedCornerShape(14.dp)).padding(14.dp),
                    ) { inner() }
                },
            )
        },
        confirmButton = { PillButton(confirm, primary = true) { if (value.text.isNotBlank()) onConfirm(value.text) } },
        dismissButton = { PillButton("Cancel", primary = false, onClick = onDismiss) },
    )
}

@Composable
fun ConfirmDialog(title: String, body: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Color(0xFF111111),
        shape = RoundedCornerShape(24.dp),
        title = { Text(title, fontSize = 16.sp, fontWeight = FontWeight.Bold) },
        text = { Text(body, fontSize = 13.sp, lineHeight = 20.sp, color = C.Text2) },
        confirmButton = { PillButton(confirm, primary = true, onClick = onConfirm) },
        dismissButton = { PillButton("Cancel", primary = false, onClick = onDismiss) },
    )
}

@Composable
fun MoveDialog(root: File, folders: List<File>, exclude: List<File>, onDismiss: () -> Unit, onPick: (File) -> Unit) {
    val choices = folders.filter { f ->
        exclude.none { ex -> f == ex || f.absolutePath.startsWith(ex.absolutePath + "/") } &&
            exclude.none { it.parentFile == f }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = Color(0xFF111111),
        shape = RoundedCornerShape(24.dp),
        title = { Text("Move to", fontSize = 16.sp, fontWeight = FontWeight.Bold) },
        text = {
            if (choices.isEmpty()) Text("No other folders yet.", fontSize = 13.sp, color = C.Muted)
            LazyColumn(Modifier.heightIn(max = 380.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                items(choices, key = { it.absolutePath }) { f ->
                    val rel = f.relativeTo(root).path
                    val depth = if (rel.isEmpty()) 0 else rel.count { it == '/' } + 1
                    Row(
                        Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).clickable { onPick(f) }
                            .padding(start = (12 + depth * 16).dp, top = 12.dp, bottom = 12.dp, end = 12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        Icon(Icons.Folder, null, tint = C.White, modifier = Modifier.size(16.dp))
                        Text(if (rel.isEmpty()) "Noted (top level)" else f.name, fontSize = 14.sp)
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { PillButton("Cancel", primary = false, onClick = onDismiss) },
    )
}

@Composable
fun VStack(gap: Dp, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(gap)) { content() }
}
