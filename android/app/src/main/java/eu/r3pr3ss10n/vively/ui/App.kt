package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import eu.r3pr3ss10n.vively.CoreStore
import eu.r3pr3ss10n.vively.core.FfiNav
import eu.r3pr3ss10n.vively.core.FfiRecentKind
import eu.r3pr3ss10n.vively.core.VivelyCore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed interface Screen {
    data object Home : Screen
    data object Search : Screen
    data object Library : Screen
    data object Profile : Screen
    data object Friends : Screen
    data object Downloads : Screen
    data object Settings : Screen
    data class Playlist(val key: String) : Screen
    data class Offline(val fullId: String) : Screen
    data class Artist(val id: String, val name: String) : Screen
    data class Friend(val id: Long) : Screen
    data class Section(val key: String) : Screen
}

val TABS = listOf(Screen.Home, Screen.Search, Screen.Library, Screen.Profile)

/** One back stack per bottom tab. */
class Navigator {
    var tab by mutableStateOf<Screen>(Screen.Home)
        private set
    private val stacks = mutableStateMapOf<Screen, List<Screen>>()

    val stack: List<Screen> get() = stacks[tab] ?: listOf(tab)
    val current: Screen get() = stack.last()
    val canPop: Boolean get() = stack.size > 1 || tab != Screen.Home

    fun push(s: Screen) {
        if (current != s) stacks[tab] = stack + s
    }

    fun pop(): Boolean {
        if (stack.size > 1) {
            stacks[tab] = stack.dropLast(1)
            return true
        }
        if (tab != Screen.Home) {
            tab = Screen.Home
            return true
        }
        return false
    }

    /** Re-selecting the active tab returns to its root. */
    fun select(s: Screen) {
        if (tab == s) stacks[s] = listOf(s) else tab = s
    }

    fun reset() {
        stacks.clear()
        tab = Screen.Home
    }

    /** Drop screens matching [pred] from every stack (a deleted playlist). */
    fun removeWhere(pred: (Screen) -> Boolean) {
        for ((k, v) in stacks.toMap()) {
            val kept = v.filterIndexed { i, s -> i == 0 || !pred(s) }
            if (kept.size != v.size) stacks[k] = kept
        }
    }
}

enum class StageTab { Lyrics, Queue }

/** Playlist editor session: create/edit, or add tracks to `addTo`. */
data class EditorMode(val editingKey: String?, val addTo: String? = null, val addToTitle: String? = null)

/** UI controller shared by every screen. */
class AppUi(val store: CoreStore) {
    val core: VivelyCore get() = store.core
    val nav = Navigator()
    val overlay = Overlay()
    val toasts = Toasts()
    var playerOpen by mutableStateOf(false)
    var stageTab by mutableStateOf<StageTab?>(null)
    var editor by mutableStateOf<EditorMode?>(null)
    var uploadOpen by mutableStateOf(false)
    /** Filter text per screen, kept across navigation. */
    val filters = mutableStateMapOf<String, String>()

    private fun show(s: Screen) {
        overlay.close()
        playerOpen = false
        nav.push(s)
    }

    fun openPlaylist(key: String) {
        core.openPlaylist(key)
        show(Screen.Playlist(key))
    }

    fun openOffline(fullId: String) = show(Screen.Offline(fullId))

    fun openArtist(id: String, name: String) {
        core.openArtist(id)
        show(Screen.Artist(id, name))
    }

    fun openFriend(id: Long) {
        core.openFriend(id, false)
        show(Screen.Friend(id))
    }

    fun openLibrary() {
        overlay.close()
        playerOpen = false
        nav.select(Screen.Library)
    }

    fun openProfile() {
        overlay.close()
        playerOpen = false
        nav.select(Screen.Profile)
    }

    fun openFriends() = show(Screen.Friends)
    fun openDownloads() = show(Screen.Downloads)
    fun openSettings() = show(Screen.Settings)

    fun openSection(sectionId: String, title: String) =
        async({ core.openSection(sectionId, title) }) { show(Screen.Section(it)) }

    fun go(n: FfiNav) {
        when (n) {
            is FfiNav.Library -> openLibrary()
            is FfiNav.Playlist -> show(Screen.Playlist(n.key))
            is FfiNav.Artist -> show(Screen.Artist(n.id, n.name))
            is FfiNav.Friend -> show(Screen.Friend(n.id))
            is FfiNav.Section -> show(Screen.Section(n.key))
            is FfiNav.None -> {}
        }
    }

    fun openLink(id: String) = async({ core.openLink(id) }, ::go)

    fun openPlaylistOwner(key: String) = async({ core.openPlaylistOwner(key) }, ::go)

    fun openAlbum(fullId: String) =
        async({ core.openTrackAlbum(fullId) }) { k -> k?.let { show(Screen.Playlist(it)) } }

    fun openRecent(kind: FfiRecentKind) {
        when (kind) {
            is FfiRecentKind.Playlist -> openPlaylist(kind.key)
            is FfiRecentKind.Artist -> openArtist(kind.id, kind.name)
            is FfiRecentKind.Library -> openLibrary()
        }
    }

    fun openEditor(mode: EditorMode, seed: List<String> = emptyList(), fromQueue: Boolean = false) {
        overlay.close()
        playerOpen = false
        when {
            mode.addTo != null -> core.beginAddTracks()
            fromQueue -> core.beginEditorFromQueue()
            else -> core.beginEditor(mode.editingKey, seed)
        }
        editor = mode
    }

    /** Run a synchronous core query off the main thread, then apply on main. */
    fun <T> async(query: () -> T, then: (T) -> Unit) {
        store.scope.launch {
            val r = query()
            withContext(Dispatchers.Main) { then(r) }
        }
    }
}

val LocalApp = staticCompositionLocalOf<AppUi> { error("no app") }

/** Re-runs [query] off the main thread whenever [rev] (or a key) changes. */
@Composable
fun <T> snapshotOf(rev: StateFlow<Int>, vararg keys: Any?, query: () -> T): T? {
    val latest by rememberUpdatedState(query)
    val state = produceState<T?>(null, rev, *keys) {
        rev.collectLatest { value = withContext(Dispatchers.Default) { latest() } }
    }
    return state.value
}

/** Fires [onNearEnd] when the list is scrolled close to its last item. */
@Composable
fun NearEndEffect(state: LazyListState, threshold: Int = 8, onNearEnd: () -> Unit) {
    val latest by rememberUpdatedState(onNearEnd)
    val trigger by remember(state) {
        derivedStateOf {
            val info = state.layoutInfo
            val last = info.visibleItemsInfo.lastOrNull()?.index ?: 0
            if (info.totalItemsCount > 0 && last >= info.totalItemsCount - threshold) info.totalItemsCount else -1
        }
    }
    LaunchedEffect(trigger) { if (trigger >= 0) latest() }
}

/** Height of the floating dock (mini player + tab bar) over the screen content. */
val LocalDockPad = compositionLocalOf { 0.dp }

/** Bottom content padding that keeps the last row clear of the dock. */
@Composable
fun dockPad(): Dp = LocalDockPad.current + 12.dp

/** 0 until the list scrolls [start] past the top, 1 at [end]; drives [FadingTopBar]. */
@Composable
fun headerFade(state: LazyListState, start: Dp, end: Dp): Float {
    val d = LocalDensity.current
    val s = with(d) { start.toPx() }
    val e = with(d) { end.toPx() }
    val v by remember(state, s, e) {
        derivedStateOf {
            if (state.firstVisibleItemIndex > 0) 1f
            else ((state.firstVisibleItemScrollOffset - s) / (e - s)).coerceIn(0f, 1f)
        }
    }
    return v
}
