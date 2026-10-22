package app.englishspeaker.ui.components

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.SheetState
import androidx.compose.material3.Text
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import app.englishspeaker.ui.icons.Icons
import app.englishspeaker.ui.theme.LocalPalette
import app.englishspeaker.ui.theme.Type
import kotlinx.coroutines.launch

/** 底部 Sheet：surface 底，顶角 28，40×5 拖动条。 */
@Composable
fun AppSheet(
    onDismiss: () -> Unit,
    state: SheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    content: @Composable ColumnScope.(close: () -> Unit) -> Unit,
) {
    val p = LocalPalette.current
    val scope = rememberCoroutineScope()
    val close: () -> Unit = { scope.launch { state.hide() }.invokeOnCompletion { onDismiss() } }
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = state,
        containerColor = p.surface,
        contentColor = p.t1,
        scrimColor = Color.Black.copy(alpha = if (p.dark) 0.5f else 0.25f),
        shape = RoundedCornerShape(topStart = 28.dp, topEnd = 28.dp),
        dragHandle = { SheetHandle() },
        tonalElevation = 0.dp,
    ) {
        content(close)
    }
}

/** Sheet 标题行：关闭（control 底）· 标题 · 占位。 */
@Composable
fun SheetHeader(title: String, onClose: () -> Unit) {
    val p = LocalPalette.current
    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        IconBtn(Icons.line("close", 2f), "关闭", fill = true, iconSize = 20.dp, onClick = onClose)
        Box(Modifier.weight(1f), contentAlignment = Alignment.Center) { Text(title, style = Type.sheetTitle, color = p.t1) }
        Box(Modifier.size(44.dp))
    }
}
