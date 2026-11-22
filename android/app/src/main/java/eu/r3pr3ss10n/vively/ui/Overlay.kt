package eu.r3pr3ss10n.vively.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

typealias SheetContent = @Composable ColumnScope.() -> Unit

/** One bottom sheet at a time, drawn above the whole app. */
class Overlay {
    var sheet by mutableStateOf<SheetContent?>(null)
        private set

    fun open(content: SheetContent) {
        sheet = content
    }

    fun close() {
        sheet = null
    }
}

val LocalOverlay = staticCompositionLocalOf<Overlay> { error("no overlay") }

private val SheetShape = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp)

@Composable
fun SheetHost(overlay: Overlay) {
    val c = VivelyTheme.colors
    val content = overlay.sheet
    var shown by remember { mutableStateOf<SheetContent?>(null) }
    if (content != null && shown !== content) shown = content
    BackHandler(enabled = content != null) { overlay.close() }
    Box(Modifier.fillMaxSize()) {
        AnimatedVisibility(content != null, enter = fadeIn(tween(200)), exit = fadeOut(tween(180))) {
            Box(Modifier.fillMaxSize().background(c.scrim).pointerInput(Unit) { detectTapGestures { overlay.close() } })
        }
        AnimatedVisibility(
            content != null,
            modifier = Modifier.align(Alignment.BottomCenter),
            enter = slideInVertically(tween(260)) { it },
            exit = slideOutVertically(tween(200)) { it },
        ) {
            BoxWithConstraints(Modifier.fillMaxWidth()) {
                val maxH = maxHeight
                var drag by remember { mutableFloatStateOf(0f) }
                Column(
                    Modifier
                        .fillMaxWidth()
                        .heightIn(max = maxH * 0.92f)
                        .graphicsLayer { translationY = drag.coerceAtLeast(0f) }
                        .shadow(18.dp, SheetShape)
                        .clip(SheetShape)
                        .background(c.raised)
                        .pointerInput(Unit) { detectTapGestures { } }
                        .navigationBarsPadding()
                        .imePadding()
                        .padding(bottom = 10.dp),
                ) {
                    Box(
                        Modifier
                            .fillMaxWidth()
                            .pointerInput(Unit) {
                                detectVerticalDragGestures(
                                    onDragEnd = {
                                        if (drag > 110.dp.toPx()) overlay.close()
                                        drag = 0f
                                    },
                                    onDragCancel = { drag = 0f },
                                ) { _, dy -> drag += dy }
                            }
                            .padding(top = 10.dp, bottom = 14.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Box(Modifier.size(36.dp, 4.dp).clip(CircleShape).background(c.fill2))
                    }
                    shown?.invoke(this)
                }
            }
        }
    }
}

class ToastItem(val id: Long, val message: String)

class Toasts {
    val items = mutableStateListOf<ToastItem>()
    private var next = 0L

    fun show(message: String) {
        items += ToastItem(next++, message)
        while (items.size > 3) items.removeAt(0)
    }
}

@Composable
fun ToastHost(toasts: Toasts, modifier: Modifier = Modifier) {
    val c = VivelyTheme.colors
    Column(modifier, horizontalAlignment = Alignment.CenterHorizontally) {
        for (t in toasts.items) {
            key(t.id) {
                LaunchedEffect(t.id) {
                    delay(3200)
                    toasts.items.remove(t)
                }
                Box(
                    Modifier
                        .padding(bottom = 8.dp, start = 24.dp, end = 24.dp)
                        .shadow(12.dp, RL)
                        .clip(RL)
                        .background(c.ink)
                        .padding(horizontal = 18.dp, vertical = 11.dp),
                ) {
                    VText(t.message, color = c.bg, size = 14.sp, weight = FontWeight.SemiBold)
                }
            }
        }
    }
}
