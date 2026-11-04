package app.noted

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import app.noted.ui.C
import app.noted.ui.FolderScreen
import app.noted.ui.NoteScreen
import app.noted.ui.NotedTheme
import app.noted.ui.PillButton
import app.noted.ui.SearchScreen
import java.io.File

class MainActivity : ComponentActivity() {
    private val vm: AppViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(C.Black.toArgb()),
            navigationBarStyle = SystemBarStyle.dark(C.Black.toArgb()),
        )
        super.onCreate(savedInstanceState)
        lifecycle.addObserver(LifecycleEventObserver { _, e -> if (e == Lifecycle.Event.ON_RESUME) vm.onResume() })
        setContent {
            NotedTheme {
                Box(Modifier.fillMaxSize().background(C.Black)) { App(vm) }
            }
        }
    }
}

@Composable
fun App(vm: AppViewModel) {
    val top = vm.stack.lastOrNull() ?: return
    BackHandler(enabled = vm.stack.size > 1) { vm.pop() }
    val holder = rememberSaveableStateHolder()
    holder.SaveableStateProvider(top.stateKey()) {
        when (top) {
            Screen.Setup -> SetupScreen(vm)
            is Screen.Folder -> FolderScreen(vm, top.dir)
            is Screen.Note -> NoteScreen(vm, top)
            Screen.Search -> SearchScreen(vm)
        }
    }
}

private fun Screen.stateKey(): String = when (this) {
    Screen.Setup -> "setup"
    is Screen.Folder -> "folder:" + dir.absolutePath
    is Screen.Note -> "note:" + file.absolutePath + editing
    Screen.Search -> "search"
}

/** Turns a SAF tree uri (primary:Documents/Notes) into a real path. */
private fun treeUriToFile(uri: Uri): File? {
    val id = DocumentsContract.getTreeDocumentId(uri) ?: return null
    val parts = id.split(":", limit = 2)
    val vol = parts[0]
    val rel = parts.getOrElse(1) { "" }
    val base = if (vol.equals("primary", true)) Environment.getExternalStorageDirectory().absolutePath else "/storage/$vol"
    return if (rel.isEmpty()) File(base) else File(base, rel)
}

@Composable
private fun SetupScreen(vm: AppViewModel) {
    val ctx = androidx.compose.ui.platform.LocalContext.current
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocumentTree()) { uri ->
        uri?.let(::treeUriToFile)?.let { vm.chooseRoot(it) }
    }
    val default = File(Environment.getExternalStorageDirectory(), "Noted")
    Column(
        Modifier.fillMaxSize().safeDrawingPadding().padding(horizontal = 24.dp, vertical = 32.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Spacer(Modifier.height(24.dp))
        Text("Noted", fontSize = 26.sp, fontWeight = FontWeight.Bold)
        Text(
            "Your notes are plain .md files in a folder on your phone. Folders here are real folders, " +
                "so you can point a sync app (FolderSync, Autosync for Google Drive) at the same folder.",
            fontSize = 14.sp, lineHeight = 22.sp, color = C.Text2,
        )
        Spacer(Modifier.height(8.dp))
        if (!vm.hasAccess) {
            Step(
                "1", "Allow file access",
                "Android needs \"All files access\" so Noted can read and write .md files in any folder you choose.",
            )
            PillButton("Open settings", primary = true) {
                ctx.startActivity(
                    Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:" + ctx.packageName)),
                )
            }
        } else {
            Step("2", "Pick your notes folder", "Use an existing folder (an Obsidian vault works) or start fresh.")
            PillButton("Use ${default.absolutePath.removePrefix(Environment.getExternalStorageDirectory().absolutePath).ifEmpty { "/" }}", primary = true) {
                vm.chooseRoot(default)
            }
            PillButton("Choose another folder", primary = false) { picker.launch(null) }
            vm.root?.let {
                Text("Current: ${it.absolutePath}", fontSize = 11.sp, color = C.Muted)
            }
        }
    }
}

@Composable
private fun Step(n: String, title: String, body: String) {
    Column(
        Modifier.fillMaxWidth().border(1.dp, C.Border, RoundedCornerShape(20.dp)).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(title, fontSize = 14.sp, fontWeight = FontWeight.Bold)
        Text(body, fontSize = 12.sp, lineHeight = 19.sp, color = C.Text3)
    }
}
