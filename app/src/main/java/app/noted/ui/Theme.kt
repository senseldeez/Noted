package app.noted.ui

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.noted.R

object C {
    val Black = Color(0xFF000000)
    val White = Color(0xFFFFFFFF)
    val Border = Color(0xFF2A2A2A)
    val Divider = Color(0xFF1C1C1C)
    val Pill = Color(0xFF161616)
    val Toolbar = Color(0xFF1C1C1C)
    val Text2 = Color(0xFFD6D6D6)
    val Text3 = Color(0xFFB5B5B5)
    val Muted = Color(0xFF8F8F8F)
    val Syntax = Color(0xFF7A7A7A)
    val Guide = Color(0xFF262626)
    val CodeBg = Color(0xFF0E0E0E)
}

val Mono = FontFamily(
    Font(R.font.jetbrains_mono_regular, FontWeight.Normal),
    Font(R.font.jetbrains_mono_medium, FontWeight.Medium),
    Font(R.font.jetbrains_mono_bold, FontWeight.Bold),
    Font(R.font.jetbrains_mono_italic, FontWeight.Normal, FontStyle.Italic),
)

private fun t(size: Int, weight: FontWeight = FontWeight.Normal, lh: Float = 1.5f) =
    TextStyle(fontFamily = Mono, fontSize = size.sp, fontWeight = weight, lineHeight = (size * lh).sp, color = C.White)

@Composable
fun NotedTheme(content: @Composable () -> Unit) {
    val ty = Typography(
        displayLarge = t(32, FontWeight.Bold), displayMedium = t(28, FontWeight.Bold), displaySmall = t(26, FontWeight.Bold),
        headlineLarge = t(26, FontWeight.Bold), headlineMedium = t(22, FontWeight.Bold), headlineSmall = t(20, FontWeight.Bold),
        titleLarge = t(18, FontWeight.Bold), titleMedium = t(15, FontWeight.Bold), titleSmall = t(14, FontWeight.Bold),
        bodyLarge = t(15), bodyMedium = t(14), bodySmall = t(12),
        labelLarge = t(14, FontWeight.Medium), labelMedium = t(12, FontWeight.Medium), labelSmall = t(11, FontWeight.Medium),
    )
    MaterialTheme(
        colorScheme = darkColorScheme(
            primary = C.White, onPrimary = C.Black,
            secondary = C.White, onSecondary = C.Black,
            background = C.Black, onBackground = C.White,
            surface = Color(0xFF111111), onSurface = C.White,
            surfaceVariant = C.Pill, onSurfaceVariant = C.Text3,
            surfaceContainer = Color(0xFF111111), surfaceContainerHigh = Color(0xFF141414),
            surfaceContainerHighest = Color(0xFF1A1A1A), surfaceContainerLow = Color(0xFF0C0C0C),
            outline = C.Border, outlineVariant = C.Divider,
            error = Color(0xFFFF8A80),
        ),
        typography = ty,
        content = content,
    )
}

/* Icons, drawn from the same SVG paths as the mockups (24x24 viewBox). */
object Icons {
    private fun build(name: String, vararg parts: Pair<String, Boolean>, stroke: Float = 1.8f): ImageVector {
        val b = ImageVector.Builder(name, 24.dp, 24.dp, 24f, 24f)
        for ((d, filled) in parts) {
            val nodes = PathParser().parsePathString(d).toNodes()
            if (filled) b.addPath(nodes, fill = SolidColor(Color.White))
            else b.addPath(
                nodes, stroke = SolidColor(Color.White), strokeLineWidth = stroke,
                strokeLineCap = StrokeCap.Round, strokeLineJoin = StrokeJoin.Round,
            )
        }
        return b.build()
    }

    private fun circle(cx: Float, cy: Float, r: Float) = "M${cx - r},${cy}a$r,$r 0 1,0 ${2 * r},0a$r,$r 0 1,0 ${-2 * r},0"

    val Search = build("search", circle(11f, 11f, 7f) to false, "M20 20l-4-4" to false)
    val More = build("more", circle(12f, 5f, 1.7f) to true, circle(12f, 12f, 1.7f) to true, circle(12f, 19f, 1.7f) to true)
    val MoreH = build("moreh", circle(5f, 12f, 1.7f) to true, circle(12f, 12f, 1.7f) to true, circle(19f, 12f, 1.7f) to true)
    val Plus = build("plus", "M12 5v14M5 12h14" to false, stroke = 2.2f)
    val Back = build("back", "M15 5l-7 7 7 7" to false)
    val Close = build("close", "M6 6l12 12M18 6L6 18" to false)
    val Folder = build("folder", "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" to true)
    val FolderOutline = build("folderOutline", "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" to false)
    val PinFilled = build("pinFilled", "M9 3h6l-1 7 4 4H6l4-4z" to true, "M12 14v7" to false, stroke = 2f)
    val Pin = build("pin", "M9 3h6l-1 7 4 4H6l4-4z" to false, "M12 14v7" to false)
    val Move = build("move", "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" to false, "M10 13h6M13 10l3 3-3 3" to false)
    val Delete = build("delete", "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" to false)
    val Edit = build("edit", "M4 20h4L19 9l-4-4L4 16z" to false, "M13 7l4 4" to false, stroke = 2f)
    val Rename = build("rename", "M4 20h4L19 9l-4-4L4 16z" to false, "M13 7l4 4" to false)
    val Undo = build("undo", "M9 14L4 9l5-5" to false, "M4 9h10a6 6 0 0 1 0 12h-3" to false)
    val Redo = build("redo", "M15 14l5-5-5-5" to false, "M20 9H10a6 6 0 0 0 0 12h3" to false)
    val Chevron = build("chevron", "M9 6l6 6-6 6" to false, stroke = 2.4f)
    val Bullets = build(
        "bullets", circle(5f, 7f, 1.3f) to true, circle(5f, 12f, 1.3f) to true, circle(5f, 17f, 1.3f) to true,
        "M10 7h10M10 12h10M10 17h10" to false, stroke = 2f,
    )
    val Numbers = build("numbers", "M4 5h1.5v4M4 9h3M4 14.5a1.5 1.5 0 0 1 3 0c0 1.5-3 2-3 3.5h3M10 7h10M10 12h10M10 17h10" to false, stroke = 2f)
    val Checkbox = build("checkbox", "M8 4h8a4 4 0 0 1 4 4v8a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4V8a4 4 0 0 1 4-4z" to false, "M8.5 12l2.5 2.5 4.5-5" to false, stroke = 2f)
    val Indent = build("indent", "M20 6H10M20 12h-7M20 18H10M4 9l3 3-3 3" to false, stroke = 2f)
    val Outdent = build("outdent", "M20 6H10M20 12h-7M20 18H10M7 9l-3 3 3 3" to false, stroke = 2f)
    val Quote = build("quote", "M5 6v12" to false, "M10 8h9M10 12h9M10 16h6" to false, stroke = 2f)
    val Code = build("code", "M8 7l-5 5 5 5M16 7l5 5-5 5" to false, stroke = 2f)
    val Link = build("link", "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1" to false, "M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1" to false, stroke = 2f)
    val Rule = build("rule", "M4 12h16" to false, stroke = 2f)
    val Checkmark = build("check", "M6 12.5l4 4 8-9" to false, stroke = 2.8f)
    val Note = build("note", "M7 3h7l5 5v13H7a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" to false, "M14 3v5h5" to false)
}
