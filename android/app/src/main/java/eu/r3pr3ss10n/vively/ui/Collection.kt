package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiLoad
import eu.r3pr3ss10n.vively.core.FfiPlaylistDetail
import eu.r3pr3ss10n.vively.core.formatTotalDuration
import eu.r3pr3ss10n.vively.core.formatTracksCount

@Composable
fun PlaylistScreen(key: String) {
    val app = LocalApp.current
    val core = app.core
    LaunchedEffect(key) { core.openPlaylist(key) }
    val detail = snapshotOf(app.store.pagesRev, key) { core.playlistDetail(key) }
    CollectionContent(
        detail = detail,
        listKey = "pl:$key",
        offline = false,
        onReload = { core.reloadPlaylist(key) },
        onLoadMore = { core.loadMorePlaylistTracks(key) },
    )
}

@Composable
fun OfflineCollectionScreen(fullId: String) {
    val app = LocalApp.current
    val core = app.core
    val detail = snapshotOf(app.store.downloadsRev, fullId) { core.offlineCollection(fullId) }
    CollectionContent(
        detail = detail,
        listKey = "off:$fullId",
        offline = true,
        onReload = {},
        onLoadMore = {},
    )
}

@Composable
private fun CollectionContent(
    detail: FfiPlaylistDetail?,
    listKey: String,
    offline: Boolean,
    onReload: () -> Unit,
    onLoadMore: () -> Unit,
) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val auth by app.store.auth.collectAsState()
    val now = rememberNowPlaying()
    val list = rememberLazyListState()

    if (detail == null) {
        Column(Modifier.fillMaxSize()) {
            TopBar("", false, onBack = { app.nav.pop() })
            if (offline) {
                EmptyState(VIcons.Album, "Коллекция не найдена", "Этот плейлист не найден среди загруженных")
            } else {
                TrackListSkeleton(8)
            }
        }
        return
    }

    val pl = detail.playlist
    val colTint = rememberArtworkTint(pl.artworkUrl, pl.tintHex)
    val isOwn = pl.ownerId == auth.userId || pl.editable
    val loadingInitial = (detail.state is FfiLoad.Idle || detail.state is FfiLoad.Loading) && detail.tracks.isEmpty()
    val total = maxOf(detail.total, detail.tracks.size.toLong())
    val hasMore = !offline && detail.tracks.size.toLong() < detail.total

    val metaLine = remember(pl) {
        buildList {
            if (pl.isAlbum) add("Альбом")
            if (pl.hidden) add("Скрытый")
            pl.genre?.takeIf { it.isNotBlank() }?.let { add(it) }
            pl.year?.takeIf { it > 0 }?.let { add(it.toString()) }
        }.joinToString(" · ")
    }
    val counts = remember(total, detail.totalSecs) {
        buildList {
            if (total > 0) add(formatTracksCount(total))
            if (detail.totalSecs > 0) add(formatTotalDuration(detail.totalSecs))
        }
    }

    val cleanByline = remember(pl, isOwn) {
        val raw = pl.artistName?.takeIf { pl.isAlbum && it.isNotBlank() }
            ?: pl.byline.ifBlank { if (isOwn) "Мой плейлист" else "" }
        val yr = pl.year?.takeIf { it > 0 }?.toString()
        if (yr != null) raw.removeSuffix(" · $yr").trim() else raw
    }

    NearEndEffect(list) {
        if (hasMore && !detail.loadingMore) onLoadMore()
    }

    val heroTop = c.heroTop(colTint)
    val fade = headerFade(list, 200.dp, 300.dp)
    val canPlay = detail.tracks.isNotEmpty()
    val play = {
        when {
            offline -> core.playOfflineCollection(pl.fullId, 0u, false)
            else -> core.playPlaylist(pl.key, false)
        }
    }
    val shuffle = {
        when {
            offline -> core.playOfflineCollection(pl.fullId, 0u, true)
            else -> core.playTracksShuffled(detail.tracks.map { it.fullId }, pl.title)
        }
    }

    Box(Modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            state = list,
            contentPadding = PaddingValues(bottom = dockPad()),
        ) {
            item(key = "hero") {
                Column(
                    Modifier
                        .fillMaxWidth()
                        .background(Brush.verticalGradient(listOf(heroTop, lerp(heroTop, c.bg, 0.55f), c.bg)))
                        .statusBarsPadding()
                        .padding(top = 56.dp),
                ) {
                    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                        Box(Modifier.shadow(28.dp, RoundedCornerShape(10.dp))) {
                            Cover(pl.artworkUrl, 232.dp, radius = 10.dp)
                        }
                    }
                    Spacer(Modifier.height(22.dp))
                    Column(Modifier.fillMaxWidth().padding(horizontal = 24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                        VText(
                            pl.title,
                            size = 21.sp,
                            weight = FontWeight.Bold,
                            lineHeight = 25.sp,
                            maxLines = 2,
                            align = TextAlign.Center,
                            tracking = (-0.2).sp,
                        )
                        if (cleanByline.isNotEmpty()) {
                            val canOpenArtist = pl.isAlbum && pl.artistId != null
                            val canOpenOwner = !offline && !pl.isAlbum && detail.ownerMusicId != null
                            val byMod = when {
                                canOpenArtist -> Modifier.tap { app.openArtist(pl.artistId!!, pl.artistName ?: cleanByline) }
                                canOpenOwner -> Modifier.tap { app.openPlaylistOwner(pl.key) }
                                else -> Modifier
                            }
                            VText(
                                cleanByline,
                                Modifier.padding(top = 4.dp).then(byMod),
                                color = c.ink2,
                                size = 15.sp,
                                weight = FontWeight.Normal,
                                maxLines = 1,
                                align = TextAlign.Center,
                            )
                        }
                        if (metaLine.isNotEmpty()) {
                            VText(metaLine, Modifier.padding(top = 5.dp), color = c.ink2, size = 14.sp, lineHeight = 19.sp, maxLines = 2, align = TextAlign.Center)
                        }
                        pl.description?.takeIf { it.isNotBlank() }?.let { desc ->
                            VText(desc, Modifier.padding(top = 8.dp), color = c.ink2, size = 14.sp, lineHeight = 20.sp, maxLines = 3, align = TextAlign.Center)
                        }
                    }
                    Spacer(Modifier.height(10.dp))
                    CollectionActions(
                        onPlay = if (canPlay) play else null,
                        onShuffle = if (canPlay) shuffle else null,
                        info = counts,
                        trailing = {
                            if (!offline) {
                                if (pl.editable) {
                                    ActionIcon(VIcons.Plus, { app.openEditor(EditorMode(null, addTo = pl.key, addToTitle = pl.title)) })
                                    ActionIcon(VIcons.Edit, { app.openEditor(EditorMode(pl.key)) })
                                } else {
                                    ActionIcon(
                                        if (pl.following) VIcons.Check else VIcons.Plus,
                                        { core.toggleFollowPlaylist(pl.key) },
                                        tint = if (pl.following) c.hi else c.ink2,
                                    )
                                }
                                ActionIcon(
                                    if (detail.downloaded) VIcons.Downloaded else VIcons.Download,
                                    { core.toggleDownloadPlaylist(pl.key) },
                                    tint = if (detail.downloaded) c.hi else c.ink2,
                                )
                                if (pl.editable || pl.canCopy) {
                                    ActionIcon(VIcons.More, { app.overlay.open { PlaylistPageSheet(pl) } })
                                }
                            }
                        },
                    )
                    Spacer(Modifier.height(8.dp))
                }
            }

            when {
                loadingInitial -> item(key = "skel") { TrackListSkeleton(8) }
                detail.tracks.isEmpty() && detail.state is FfiLoad.Failed -> item(key = "err") {
                    ErrorState("Не удалось загрузить плейлист", (detail.state as FfiLoad.Failed).message, onReload)
                }
                detail.tracks.isEmpty() -> item(key = "empty") {
                    EmptyState(
                        VIcons.Album,
                        "Плейлист пуст",
                        if (pl.editable && !offline) "Добавьте треки, чтобы собрать плейлист" else "В этом плейлисте пока нет треков",
                        action = if (pl.editable && !offline) {
                            {
                                PrimaryPill(VIcons.Plus, "Добавить треки") {
                                    app.openEditor(EditorMode(null, addTo = pl.key, addToTitle = pl.title))
                                }
                            }
                        } else null,
                    )
                }
                else -> {
                    val editKey = if (pl.editable && !offline) pl.key else null
                    trackItems(
                        list = detail.tracks,
                        now = now,
                        label = pl.title,
                        keyPrefix = listKey,
                        editableKey = editKey,
                        numbered = pl.isAlbum,
                        onPlay = { i ->
                            if (offline) core.playOfflineCollection(pl.fullId, i.toUInt(), false)
                            else core.playTracks(detail.tracks.map { it.fullId }, i.toUInt(), pl.title)
                        },
                    )
                    if (detail.loadingMore) {
                        item(key = "more") {
                            Box(Modifier.fillMaxWidth().padding(vertical = 18.dp), contentAlignment = Alignment.Center) {
                                Spinner(20.dp, c.ink2)
                            }
                        }
                    } else if (detail.moreFailed) {
                        item(key = "more-err") {
                            Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                                Pill("Повторить загрузку", onClick = onLoadMore)
                            }
                        }
                    }
                }
            }
        }
        FadingTopBar(pl.title, fade, lerp(heroTop, c.bg, 0.35f), onBack = { app.nav.pop() })
    }
}
