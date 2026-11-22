package eu.r3pr3ss10n.vively.ui

import android.content.ActivityNotFoundException
import android.content.Intent
import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.tween
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsTopHeight
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.animation.core.Animatable
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import kotlinx.coroutines.launch
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.FileProvider
import eu.r3pr3ss10n.vively.CoreStore
import eu.r3pr3ss10n.vively.core.FfiEvent
import eu.r3pr3ss10n.vively.core.FfiPlayback
import eu.r3pr3ss10n.vively.core.FfiTrack
import java.io.File

fun hexColor(hex: String?): Color? {
    val h = hex?.trim()?.removePrefix("#") ?: return null
    if (h.length != 6) return null
    return h.toLongOrNull(16)?.let { Color(0xFF000000 or it) }
}

@Composable
fun AppShell(store: CoreStore) {
    val app = remember(store) { AppUi(store) }
    val nav = app.nav
    val context = LocalContext.current
    val pb by store.playback.collectAsState()
    val saved = rememberSaveableStateHolder()

    BackHandler(enabled = app.playerOpen) { app.playerOpen = false }
    BackHandler(enabled = !app.playerOpen && nav.canPop) { nav.pop() }

    LaunchedEffect(app) {
        store.events.collect { ev ->
            when (ev) {
                is FfiEvent.Toast -> app.toasts.show(ev.message)
                is FfiEvent.PlaylistDeleted -> nav.removeWhere { it is Screen.Playlist && it.key == ev.key }
                is FfiEvent.SearchSettled ->
                    if (nav.current == Screen.Search) app.core.settleSearchHistory(ev.query, ev.generation)
                is FfiEvent.PlaylistSaved -> {
                    val wasNew = app.editor?.editingKey == null
                    app.editor = null
                    if (wasNew) app.openPlaylist(ev.key)
                }
                is FfiEvent.EditorClosed -> app.editor = null
                is FfiEvent.UploadFinished -> if (ev.allDone) {
                    app.uploadOpen = false
                    app.core.clearUpload()
                }
                is FfiEvent.ExportFinished -> {
                    val path = ev.path
                    if (path == null) {
                        app.toasts.show(ev.error ?: "Не удалось подготовить файл")
                    } else {
                        val uri = FileProvider.getUriForFile(context, "${context.packageName}.files", File(path))
                        val send = Intent(Intent.ACTION_SEND)
                            .setType("audio/mpeg")
                            .putExtra(Intent.EXTRA_STREAM, uri)
                            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                        try {
                            context.startActivity(Intent.createChooser(send, "Поделиться файлом").addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                        } catch (_: ActivityNotFoundException) {
                            app.toasts.show("Нет приложений для отправки файла")
                        }
                    }
                }
                else -> {}
            }
        }
    }

    CompositionLocalProvider(LocalApp provides app, LocalOverlay provides app.overlay) {
        val c = VivelyTheme.colors
        val density = LocalDensity.current
        var dockPx by remember { mutableIntStateOf(0) }
        val dockDp = with(density) { dockPx.toDp() }
        Box(Modifier.fillMaxSize().background(c.bg)) {
            CompositionLocalProvider(LocalDockPad provides dockDp) {
                Box(Modifier.fillMaxSize().imePadding()) {
                    val screen = nav.current
                    saved.SaveableStateProvider("${nav.tab}/${nav.stack.size}/$screen") {
                        ScreenContent(screen)
                    }
                    if (screen in TABS) {
                        Box(Modifier.fillMaxWidth().windowInsetsTopHeight(WindowInsets.statusBars).background(c.bg))
                    }
                }
            }
            Column(
                Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .onSizeChanged { dockPx = it.height },
            ) {
                Box(Modifier.fillMaxWidth().height(28.dp).background(Brush.verticalGradient(listOf(c.bg.copy(alpha = 0f), c.bg))))
                Column(Modifier.fillMaxWidth().background(c.bg)) {
                    pb.current?.let { MiniPlayer(it, pb) }
                    BottomBar(nav)
                }
            }
            ToastHost(app.toasts, Modifier.align(Alignment.BottomCenter).padding(bottom = dockDp))
            AnimatedVisibility(
                app.playerOpen && pb.current != null,
                enter = slideInVertically(tween(300)) { it },
                exit = slideOutVertically(tween(240)) { it },
            ) { PlayerScreen() }
            app.editor?.let { mode -> EditorScreen(mode) }
            if (app.uploadOpen) UploadScreen()
            SheetHost(app.overlay)
        }
    }
}

@Composable
private fun ScreenContent(screen: Screen) {
    when (screen) {
        Screen.Home -> HomeScreen()
        Screen.Search -> SearchScreen()
        Screen.Library -> LibraryScreen()
        Screen.Profile -> ProfileScreen()
        Screen.Friends -> FriendsScreen()
        Screen.Downloads -> DownloadsScreen()
        Screen.Settings -> SettingsScreen()
        is Screen.Playlist -> PlaylistScreen(screen.key)
        is Screen.Offline -> OfflineCollectionScreen(screen.fullId)
        is Screen.Artist -> ArtistScreen(screen.id, screen.name)
        is Screen.Friend -> FriendScreen(screen.id)
        is Screen.Section -> SectionScreen(screen.key)
    }
}

private data class TabSpec(val screen: Screen, val icon: ImageVector, val active: ImageVector, val label: String)

private val TAB_SPECS = listOf(
    TabSpec(Screen.Home, VIcons.Home, VIcons.HomeFill, "Главная"),
    TabSpec(Screen.Search, VIcons.Search, VIcons.SearchBold, "Поиск"),
    TabSpec(Screen.Library, VIcons.Heart, VIcons.HeartFill, "Моя музыка"),
    TabSpec(Screen.Profile, VIcons.User, VIcons.UserFill, "Профиль"),
)

@Composable
private fun BottomBar(nav: Navigator) {
    val c = VivelyTheme.colors
    Row(Modifier.fillMaxWidth().navigationBarsPadding().height(58.dp)) {
        TAB_SPECS.forEach { t ->
            val sel = nav.tab == t.screen
            val tint = if (sel) c.ink else c.ink2
            Column(
                Modifier.weight(1f).fillMaxHeight().tap(shape = Square, pressed = Color.Transparent) { nav.select(t.screen) },
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                VIcon(if (sel) t.active else t.icon, 24.dp, tint)
                VText(
                    t.label,
                    Modifier.padding(top = 3.dp),
                    color = tint,
                    size = 11.sp,
                    weight = if (sel) FontWeight.SemiBold else FontWeight.Medium,
                    maxLines = 1,
                )
            }
        }
    }
}

/** Mini player above the tab bar: tap or swipe up opens the player, swipe sideways skips. */
@Composable
private fun MiniPlayer(track: FfiTrack, pb: FfiPlayback) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val f = if (pb.durationSecs > 0) (pb.positionSecs / pb.durationSecs).toFloat().coerceIn(0f, 1f) else 0f
    val shape = RoundedCornerShape(12.dp)
    val swipe = remember { Animatable(0f) }
    val hasNext by rememberUpdatedState(pb.hasNext)
    val skipPx = with(density) { 72.dp.toPx() }
    val openPx = with(density) { 36.dp.toPx() }
    var lift by remember { mutableFloatStateOf(0f) }

    Box(
        Modifier
            .padding(start = 8.dp, end = 8.dp, bottom = 2.dp)
            .fillMaxWidth()
            .height(60.dp)
            .clip(shape)
            .background(c.dock)
            .pointerInput(Unit) {
                detectVerticalDragGestures(
                    onDragEnd = {
                        if (lift < -openPx) app.playerOpen = true
                        lift = 0f
                    },
                    onDragCancel = { lift = 0f },
                ) { _, dy -> lift += dy }
            }
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragEnd = {
                        val v = swipe.value
                        when {
                            v < -skipPx && hasNext -> app.core.nextTrack()
                            v > skipPx -> app.core.prevTrack()
                        }
                        scope.launch { swipe.animateTo(0f, tween(220)) }
                    },
                    onDragCancel = { scope.launch { swipe.animateTo(0f, tween(220)) } },
                ) { _, dx -> scope.launch { swipe.snapTo(swipe.value + dx * 0.6f) } }
            }
            .tap(shape = shape, pressed = c.fill) { app.playerOpen = true },
    ) {
        Row(
            Modifier
                .fillMaxSize()
                .graphicsLayer {
                    translationX = swipe.value
                    alpha = 1f - (kotlin.math.abs(swipe.value) / (skipPx * 3f)).coerceIn(0f, 0.6f)
                }
                .padding(start = 8.dp, end = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Cover(track.thumbUrl, 44.dp)
            Column(Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    VText(track.title, Modifier.weight(1f, fill = false), size = 14.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
                    if (track.explicit) ExplicitBadge()
                }
                VText(track.artist, color = c.ink2, size = 13.sp, maxLines = 1)
            }
            IconBtn(
                if (track.liked) VIcons.HeartFill else VIcons.Heart,
                { app.core.toggleLike(track.fullId) },
                size = 42.dp,
                iconSize = 22.dp,
                tint = c.ink,
            )
            Box(
                Modifier.size(42.dp).clip(CircleShape).tap(shape = CircleShape, pressed = c.fill) { app.core.togglePlayPause() },
                contentAlignment = Alignment.Center,
            ) {
                if (pb.loading) Spinner(22.dp, c.ink) else VIcon(if (pb.playing) VIcons.Pause else VIcons.Play, 26.dp, c.ink)
            }
        }
        Box(
            Modifier
                .align(Alignment.BottomStart)
                .padding(horizontal = 10.dp)
                .fillMaxWidth()
                .height(2.dp)
                .clip(CircleShape)
                .background(c.ink.copy(alpha = 0.14f)),
        ) {
            Box(Modifier.fillMaxWidth(f).fillMaxHeight().background(c.ink.copy(alpha = 0.85f)))
        }
    }
}
