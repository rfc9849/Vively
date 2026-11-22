package eu.r3pr3ss10n.vively.ui

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.snapping.rememberSnapFlingBehavior
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiArtistRef
import eu.r3pr3ss10n.vively.core.FfiPlaylist
import eu.r3pr3ss10n.vively.core.FfiSection
import eu.r3pr3ss10n.vively.core.FfiTrack
import eu.r3pr3ss10n.vively.core.formatDuration
import eu.r3pr3ss10n.vively.core.formatTracksCount

data class NowPlaying(val id: String?, val playing: Boolean, val loading: Boolean)

@Composable
fun rememberNowPlaying(): NowPlaying {
    val app = LocalApp.current
    val pb by app.store.playback.collectAsState()
    return NowPlaying(pb.current?.fullId, pb.playing, pb.loading)
}

/** Three-bar equaliser on the playing row. */
@Composable
fun EqBars(playing: Boolean, color: Color = VivelyTheme.colors.hi) {
    val t = rememberInfiniteTransition(label = "eq")
    val a by t.animateFloat(0.35f, 1f, infiniteRepeatable(tween(420), RepeatMode.Reverse), label = "a")
    val b by t.animateFloat(1f, 0.4f, infiniteRepeatable(tween(530), RepeatMode.Reverse), label = "b")
    val d by t.animateFloat(0.55f, 0.95f, infiniteRepeatable(tween(360), RepeatMode.Reverse), label = "d")
    val hs = if (playing) listOf(a, b, d) else listOf(0.55f, 0.8f, 0.45f)
    Row(Modifier.height(14.dp), horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.Bottom) {
        hs.forEach { Box(Modifier.width(3.dp).height(14.dp * it).background(color)) }
    }
}

val Square = RectangleShape

/** Playing overlay on a row cover: equaliser, or a spinner while buffering. */
@Composable
fun NowPlayingBadge(now: NowPlaying, size: Dp) {
    Box(
        Modifier.size(size).clip(RoundedCornerShape(6.dp)).background(Color(0x8C000000)),
        contentAlignment = Alignment.Center,
    ) {
        if (now.loading) Spinner(18.dp, Color.White) else EqBars(now.playing, Color.White)
    }
}

/** Track row: cover, title/artist, duration, menu. Long-press opens the menu too. */
@Composable
fun TrackRow(
    track: FfiTrack,
    now: NowPlaying,
    onPlay: () -> Unit,
    modifier: Modifier = Modifier,
    index: Int? = null,
    editableKey: String? = null,
    showCover: Boolean = true,
    showArtist: Boolean = true,
    showLiked: Boolean = true,
) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val current = now.id == track.fullId
    val openMenu = { app.overlay.open { TrackSheet(track, editableKey) } }
    Row(
        modifier
            .fillMaxWidth()
            .height(68.dp)
            .tap(shape = Square, onLongClick = openMenu, onClick = onPlay)
            .padding(start = 16.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (showCover) {
            Box(contentAlignment = Alignment.Center) {
                Cover(track.thumbUrl, 48.dp)
                if (current) NowPlayingBadge(now, 48.dp)
            }
        } else if (index != null) {
            Box(Modifier.width(24.dp), contentAlignment = Alignment.Center) {
                when {
                    current && now.loading -> Spinner(16.dp, c.hi)
                    current -> EqBars(now.playing)
                    else -> VText("${index + 1}", color = c.ink2, size = 15.sp, weight = FontWeight.Medium)
                }
            }
        }
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                VText(
                    track.title,
                    Modifier.weight(1f, fill = false),
                    color = when {
                        current -> c.hi
                        !track.playable -> c.ink3
                        else -> c.ink
                    },
                    size = 16.sp,
                    weight = FontWeight.Medium,
                    maxLines = 1,
                )
                track.subtitle?.let { VText(it, Modifier.weight(1f, fill = false), color = c.ink2, size = 15.sp, maxLines = 1) }
                if (track.explicit) ExplicitBadge()
            }
            val liked = showLiked && track.liked
            if (showArtist || track.downloaded || track.downloadPending || liked) {
                Row(Modifier.padding(top = 3.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                    if (liked) VIcon(VIcons.HeartFill, 14.dp, c.ink2)
                    if (track.downloaded) VIcon(VIcons.Downloaded, 15.dp, c.ink2)
                    else if (track.downloadPending) Spinner(13.dp, c.ink2)
                    if (showArtist) VText(track.artist, color = c.ink2, size = 14.5.sp, maxLines = 1)
                }
            }
        }
        VText(formatDuration(track.durationSecs.toDouble()), color = c.ink2, size = 14.sp)
        IconBtn(VIcons.More, openMenu, size = 40.dp, iconSize = 20.dp, tint = c.ink2)
    }
}

/** Track rows in a LazyColumn; tapping plays [list] from that row. */
fun LazyListScope.trackItems(
    list: List<FfiTrack>,
    now: NowPlaying,
    label: String,
    keyPrefix: String,
    editableKey: String? = null,
    numbered: Boolean = false,
    showArtist: (FfiTrack) -> Boolean = { true },
    showLiked: Boolean = true,
    onPlay: ((Int) -> Unit)? = null,
) {
    val ids = list.map { it.fullId }
    itemsIndexed(list, key = { i, t -> "$keyPrefix-$i-${t.fullId}" }) { i, t ->
        val app = LocalApp.current
        TrackRow(
            t,
            now,
            onPlay = { onPlay?.invoke(i) ?: app.core.playTracks(ids, i.toUInt(), label) },
            index = i,
            editableKey = editableKey,
            showCover = !numbered,
            showArtist = showArtist(t),
            showLiked = showLiked,
        )
    }
}

@Composable
fun PlaylistCard(p: FfiPlaylist, width: Dp = CARD_W, subtitle: String = p.byline, offline: Boolean = false, dotColor: Color? = null) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    Column(
        Modifier
            .width(width)
            .tap(
                pressed = Color.Transparent,
                onLongClick = { app.overlay.open { PlaylistSheet(p) } },
                onClick = { if (offline) app.openOffline(p.fullId) else app.openPlaylist(p.key) },
            ),
    ) {
        Cover(p.artworkUrl, width, radius = 8.dp)
        Spacer(Modifier.height(10.dp))
        VText(p.title, size = 15.sp, weight = FontWeight.SemiBold, maxLines = 1)
        Row(Modifier.padding(top = 3.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (dotColor != null) Box(Modifier.size(8.dp).clip(CircleShape).background(dotColor))
            VText(subtitle, color = c.ink2, size = 13.5.sp, maxLines = 1)
        }
    }
}

@Composable
fun PlaylistShelf(items: List<FfiPlaylist>, offline: Boolean = false) {
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        items(items, key = { it.key }) { PlaylistCard(it, offline = offline) }
    }
}

@Composable
fun ArtistCard(a: FfiArtistRef, size: Dp = 120.dp) {
    val app = LocalApp.current
    Column(Modifier.width(size).tap(pressed = Color.Transparent) { app.openArtist(a.id, a.name) }, horizontalAlignment = Alignment.CenterHorizontally) {
        Cover(a.photoUrl, size, round = true)
        Spacer(Modifier.height(10.dp))
        VText(a.name, size = 14.5.sp, weight = FontWeight.SemiBold, maxLines = 1, align = TextAlign.Center)
    }
}

@Composable
fun ArtistShelf(items: List<FfiArtistRef>) {
    LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(16.dp)) {
        items(items, key = { it.id }) { ArtistCard(it) }
    }
}

/** Horizontal 3-row snapping track columns shelf (like VK Music / Apple Music). */
@Composable
fun TrackColumnsShelf(
    tracks: List<FfiTrack>,
    now: NowPlaying,
    label: String,
    rowsPerColumn: Int = 3,
    onPlay: ((Int) -> Unit)? = null,
) {
    if (tracks.isEmpty()) return
    val app = LocalApp.current
    val ids = remember(tracks) { tracks.map { it.fullId } }
    val cols = remember(tracks, rowsPerColumn) { tracks.chunked(rowsPerColumn) }
    val listState = rememberLazyListState()
    val fling = rememberSnapFlingBehavior(listState)

    BoxWithConstraints(Modifier.fillMaxWidth()) {
        val colWidth = if (cols.size == 1) {
            (maxWidth - 32.dp).coerceAtLeast(240.dp)
        } else {
            (maxWidth - 56.dp).coerceIn(260.dp, 360.dp)
        }
        LazyRow(
            state = listState,
            flingBehavior = fling,
            contentPadding = PaddingValues(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            itemsIndexed(cols, key = { ci, col -> "tc-$ci-${col.first().fullId}" }) { ci, col ->
                Column(Modifier.width(colWidth)) {
                    col.forEachIndexed { ri, track ->
                        val absIdx = ci * rowsPerColumn + ri
                        CompactTrackRow(
                            track = track,
                            now = now,
                            onPlay = { onPlay?.invoke(absIdx) ?: app.core.playTracks(ids, absIdx.toUInt(), label) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun CompactTrackRow(
    track: FfiTrack,
    now: NowPlaying,
    onPlay: () -> Unit,
) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val current = now.id == track.fullId
    val openMenu = { app.overlay.open { TrackSheet(track, null) } }
    Row(
        Modifier
            .fillMaxWidth()
            .height(68.dp)
            .tap(shape = RS, onLongClick = openMenu, onClick = onPlay),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(contentAlignment = Alignment.Center) {
            Cover(track.thumbUrl, 48.dp)
            if (current) NowPlayingBadge(now, 48.dp)
        }
        Column(Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                VText(
                    track.title,
                    Modifier.weight(1f, fill = false),
                    color = when {
                        current -> c.hi
                        !track.playable -> c.ink3
                        else -> c.ink
                    },
                    size = 16.sp,
                    weight = FontWeight.Medium,
                    maxLines = 1,
                )
                if (track.explicit) ExplicitBadge()
            }
            Row(Modifier.padding(top = 3.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                if (track.downloaded) VIcon(VIcons.Downloaded, 14.dp, c.ink2)
                else if (track.downloadPending) Spinner(12.dp, c.ink2)
                VText(track.artist, color = c.ink2, size = 14.5.sp, maxLines = 1)
            }
        }
        IconBtn(VIcons.More, openMenu, size = 40.dp, iconSize = 20.dp)
    }
}

/** A catalog section: header with "Показать все", playlist shelf and/or tracks. */
fun LazyListScope.sectionItems(
    s: FfiSection,
    now: NowPlaying,
    key: String,
    maxTracks: Int = 12,
    useColumns: Boolean = false,
    topPadding: Dp = 24.dp,
) {
    if (s.tracks.isEmpty() && s.playlists.isEmpty()) return
    if (s.title.isNotEmpty()) {
        item(key = "$key-h") {
            val app = LocalApp.current
            SectionHeader(s.title, s.subtitle, topPadding = topPadding, onAction = s.showAllId?.let { id -> { app.openSection(id, s.title) } })
        }
    }
    if (s.playlists.isNotEmpty()) item(key = "$key-pl") { PlaylistShelf(s.playlists) }
    if (s.tracks.isNotEmpty()) {
        val label = s.title.ifEmpty { "Каталог" }
        val asCols = useColumns || s.columns
        if (asCols) {
            item(key = "$key-tc") {
                TrackColumnsShelf(s.tracks.take(maxTracks), now, label = label)
            }
        } else {
            val ids = s.tracks.map { it.fullId }
            itemsIndexed(s.tracks.take(maxTracks), key = { i, t -> "$key-t-$i-${t.fullId}" }) { i, t ->
                val app = LocalApp.current
                TrackRow(t, now, onPlay = { app.core.playTracks(ids, i.toUInt(), label) })
            }
        }
    }
}

// ---------------------------------------------------------------- sheets

@Composable
fun SheetHeader(cover: String?, title: String, sub: String, round: Boolean = false) {
    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Cover(cover, 52.dp, round = round)
        Column(Modifier.weight(1f)) {
            VText(title, size = 16.sp, weight = FontWeight.SemiBold, maxLines = 1)
            VText(sub, color = VivelyTheme.colors.ink2, size = 13.5.sp, maxLines = 1)
        }
    }
}

@Composable
private fun QuickTile(icon: ImageVector, label: String, modifier: Modifier, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Column(
        modifier.clip(RoundedCornerShape(12.dp)).background(c.fill).tap(shape = RoundedCornerShape(12.dp), pressed = c.fill2, onClick = onClick).padding(vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        VIcon(icon, 19.dp, c.ink)
        VText(label, size = 12.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
    }
}

@Composable
fun SheetDivider() = HLine(Modifier.padding(vertical = 4.dp, horizontal = 16.dp))

/** Desktop track context menu as a bottom sheet. */
@Composable
fun ColumnScope.TrackSheet(t: FfiTrack, editableKey: String?) {
    val app = LocalApp.current
    val core = app.core
    val close = { app.overlay.close() }
    val detail = if (editableKey != null) snapshotOf(app.store.pagesRev, editableKey) { core.playlistDetail(editableKey) } else null
    Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState())) {
        SheetHeader(t.thumbUrl, t.title, t.artist)
        Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            QuickTile(if (t.liked) VIcons.HeartSlash else VIcons.Heart, if (t.liked) "Убрать" else "Нравится", Modifier.weight(1f)) {
                close(); core.toggleLike(t.fullId)
            }
            QuickTile(VIcons.Queue, "Далее", Modifier.weight(1f)) { close(); core.playNext(t.fullId) }
            QuickTile(VIcons.Infinity, "Микс", Modifier.weight(1f)) { close(); core.startTrackMix(t.fullId) }
        }
        Spacer(Modifier.height(8.dp))
        MenuRow(VIcons.Close, if (t.disliked) "Снять «Не нравится»" else "Не нравится") { close(); core.toggleDislike(t.fullId) }
        MenuRow(VIcons.Plus, "Добавить в плейлист") { app.overlay.open { AddToPlaylistSheet(t) } }
        if (t.playable) {
            when {
                t.downloadPending -> MenuRow(VIcons.Close, "Отменить загрузку") { close(); core.removeDownload(t.fullId) }
                t.downloaded -> MenuRow(VIcons.Trash, "Удалить загрузку", danger = true) { close(); core.removeDownload(t.fullId) }
                else -> MenuRow(VIcons.Download, "Скачать") { close(); core.downloadTrack(t.fullId) }
            }
            MenuRow(VIcons.Share, "Поделиться файлом") { close(); core.exportTrack(t.fullId) }
        }
        if (t.albumTitle != null || t.artistId != null) {
            SheetDivider()
            t.albumTitle?.let { MenuRow(VIcons.Album, "Перейти к альбому", it) { app.openAlbum(t.fullId) } }
            t.artistId?.let { id -> MenuRow(VIcons.Mic, "Перейти к исполнителю", t.artist) { app.openArtist(id, t.artistName ?: t.artist) } }
        }
        if (editableKey != null) {
            SheetDivider()
            val list = detail?.tracks.orEmpty()
            val idx = list.indexOfFirst { it.fullId == t.fullId }
            if (idx > 0) MenuRow(VIcons.ChevUp, "Переместить выше") { close(); core.moveInPlaylist(t.fullId, editableKey, true) }
            if (idx >= 0 && idx + 1 < list.size) MenuRow(VIcons.ChevDown, "Переместить ниже") { close(); core.moveInPlaylist(t.fullId, editableKey, false) }
            MenuRow(VIcons.Trash, "Удалить из плейлиста", danger = true) { close(); core.removeFromPlaylist(t.fullId, editableKey) }
        }
    }
}

@Composable
fun ColumnScope.PlaylistSheet(p: FfiPlaylist) {
    val app = LocalApp.current
    val core = app.core
    val close = { app.overlay.close() }
    SheetHeader(p.artworkUrl, p.title, p.byline)
    MenuRow(VIcons.Play, "Слушать") { close(); core.playPlaylist(p.key, false) }
    MenuRow(VIcons.Album, "Открыть") { app.openPlaylist(p.key) }
    MenuRow(VIcons.Download, if (p.downloaded) "Удалить загрузку" else if (p.isAlbum) "Скачать альбом" else "Скачать плейлист") {
        close(); core.toggleDownloadPlaylist(p.key)
    }
    SheetDivider()
    if (p.editable) {
        MenuRow(VIcons.Plus, "Добавить треки") { app.openEditor(EditorMode(null, addTo = p.key, addToTitle = p.title)) }
        MenuRow(VIcons.Edit, "Редактировать") { app.openEditor(EditorMode(p.key)) }
        MenuRow(VIcons.Trash, "Удалить плейлист", danger = true) { app.overlay.open { ConfirmDeleteSheet(p) } }
    } else {
        MenuRow(
            if (p.following) VIcons.Trash else VIcons.Plus,
            if (p.following) "Убрать из медиатеки" else "Добавить в медиатеку",
            danger = p.following,
        ) { close(); core.toggleFollowPlaylist(p.key) }
    }
    if (p.canCopy) MenuRow(VIcons.Folder, "Сохранить копию") { close(); core.savePlaylistCopy(p.key) }
}

/** Playlist page menu: only actions the page header does not already show. */
@Composable
fun ColumnScope.PlaylistPageSheet(p: FfiPlaylist) {
    val app = LocalApp.current
    SheetHeader(p.artworkUrl, p.title, p.byline)
    if (p.canCopy) MenuRow(VIcons.Folder, "Сохранить копию") { app.overlay.close(); app.core.savePlaylistCopy(p.key) }
    if (p.editable) MenuRow(VIcons.Trash, "Удалить плейлист", danger = true) { app.overlay.open { ConfirmDeleteSheet(p) } }
}

@Composable
fun ColumnScope.ConfirmDeleteSheet(p: FfiPlaylist) {
    val app = LocalApp.current
    SheetTitle("Удалить плейлист?", "«${p.title}» будет удалён без возможности восстановления.")
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Pill("Отмена", Modifier.weight(1f)) { app.overlay.close() }
        Pill("Удалить", Modifier.weight(1f), danger = true) {
            app.overlay.close()
            app.core.deletePlaylist(p.key)
        }
    }
}

@Composable
fun ColumnScope.AddToPlaylistSheet(t: FfiTrack) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val auth by app.store.auth.collectAsState()
    LaunchedEffect(t.fullId) { core.beginAddToPlaylist() }
    val pls = snapshotOf(app.store.playlistsRev) { core.playlists() }
    var q by rememberSaveable { mutableStateOf("") }
    val own = pls?.items.orEmpty().filter { it.ownerId == auth.userId || it.editable }
    val shown = if (q.isBlank()) own else own.filter { it.title.contains(q.trim(), ignoreCase = true) }
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        VText("Добавить в плейлист", Modifier.weight(1f), size = 19.sp, weight = FontWeight.Bold)
        Pill("Новый", icon = VIcons.Plus, height = 34.dp) { app.openEditor(EditorMode(null), seed = listOf(t.fullId)) }
    }
    Row(
        Modifier.padding(horizontal = 16.dp).fillMaxWidth().clip(RL).background(c.fill).padding(12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Cover(t.thumbUrl, 40.dp)
        Column(Modifier.weight(1f)) {
            VText(t.title, size = 14.sp, weight = FontWeight.SemiBold, maxLines = 1)
            VText(t.artist, color = c.ink2, size = 12.5.sp, maxLines = 1)
        }
    }
    if (own.size > 6) {
        VField(
            q, { q = it }, "Найти плейлист",
            Modifier.padding(start = 16.dp, end = 16.dp, top = 12.dp),
            leading = VIcons.Search, height = 40.dp, onClear = { q = "" },
        )
    }
    Spacer(Modifier.height(6.dp))
    when {
        pls == null -> TrackListSkeleton(4)
        own.isEmpty() -> EmptyState(VIcons.Album, "Нет своих плейлистов", "Создайте плейлист, и трек сразу попадёт в него")
        shown.isEmpty() -> EmptyState(VIcons.Search, "Ничего не найдено", "Попробуйте другое название")
        else -> LazyColumn(Modifier.heightIn(max = 420.dp)) {
            items(shown, key = { it.key }) { p ->
                val busy = p.fullId in pls.adding
                val added = p.fullId in pls.added
                Row(
                    Modifier.fillMaxWidth()
                        .tap(enabled = !busy && !added, shape = Square) { core.addToPlaylist(t.fullId, p.key) }
                        .padding(horizontal = 16.dp, vertical = 7.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Cover(p.artworkUrl, 44.dp)
                    Column(Modifier.weight(1f)) {
                        VText(p.title, size = 15.sp, weight = FontWeight.SemiBold, maxLines = 1)
                        VText(formatTracksCount(p.count), color = c.ink2, size = 13.sp)
                    }
                    when {
                        busy -> Spinner(18.dp, c.ink2)
                        added -> VIcon(VIcons.Check, 20.dp, c.hi)
                        else -> VIcon(VIcons.Plus, 20.dp, c.ink2)
                    }
                }
            }
        }
    }
}
