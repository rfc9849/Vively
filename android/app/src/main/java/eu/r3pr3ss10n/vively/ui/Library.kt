package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiLoad
import eu.r3pr3ss10n.vively.core.formatTracksCount

@Composable
fun LibraryScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    LaunchedEffect(Unit) {
        core.loadLibrary(false)
        core.loadPlaylists(false)
    }

    val lib by app.store.library.collectAsState()
    val pls = snapshotOf(app.store.playlistsRev) { core.playlists() }
    val now = rememberNowPlaying()
    val list = rememberLazyListState()
    val total = maxOf(lib.total, lib.tracks.size.toLong())
    val loadingInitial = (lib.state is FfiLoad.Idle || lib.state is FfiLoad.Loading) && lib.tracks.isEmpty()

    NearEndEffect(list) {
        if (lib.canLoadMore && !lib.loadingMore) core.loadMoreLibrary()
    }

    LazyColumn(
        Modifier.fillMaxSize(),
        state = list,
        contentPadding = PaddingValues(bottom = dockPad()),
    ) {
        item(key = "title") {
            PageTitle("Моя музыка", if (loadingInitial) null else formatTracksCount(total)) {
                IconBtn(VIcons.More, { app.overlay.open { LibraryMenuSheet(total) } }, size = 44.dp, iconSize = 22.dp, tint = c.ink)
            }
        }

        lib.restorable?.let { removed ->
            item(key = "restore") {
                Row(
                    Modifier
                        .padding(horizontal = 16.dp, vertical = 8.dp)
                        .fillMaxWidth()
                        .clip(RL)
                        .background(c.fill)
                        .padding(start = 16.dp, end = 10.dp, top = 10.dp, bottom = 10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Column(Modifier.weight(1f)) {
                        VText("Трек удалён", size = 13.5.sp, weight = FontWeight.SemiBold)
                        VText("${removed.artist} - ${removed.title}", color = c.ink2, size = 12.5.sp, maxLines = 1)
                    }
                    Pill("Восстановить", height = 34.dp) { core.restoreRemovedTrack() }
                }
            }
        }

        item(key = "pl-h") {
            SectionHeader(
                title = "Плейлисты",
                action = "Создать",
                topPadding = 12.dp,
                onAction = { app.openEditor(EditorMode(null)) },
            )
        }
        item(key = "pl-shelf") {
            val items = pls?.items.orEmpty()
            when {
                pls == null || (items.isEmpty() && pls.state is FfiLoad.Loading) -> ShelfSkeleton()
                items.isEmpty() -> {
                    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 4.dp)) {
                        Pill("Создать первый плейлист", icon = VIcons.Plus) { app.openEditor(EditorMode(null)) }
                    }
                }
                else -> PlaylistShelf(items)
            }
        }

        item(key = "tr-h") {
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 24.dp, bottom = 0.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                VText("Треки", Modifier.weight(1f), size = 19.sp, weight = FontWeight.Bold, tracking = (-0.2).sp)
                if (lib.tracks.isNotEmpty()) {
                    IconBtn(VIcons.Shuffle, { core.playLibraryShuffled() }, size = 48.dp, iconSize = 24.dp, tint = c.ink)
                    Spacer(Modifier.width(6.dp))
                    PlayFab({ core.playLibrary(0u) })
                }
            }
        }
        when {
            loadingInitial -> item(key = "skel") { TrackListSkeleton(8) }
            lib.tracks.isEmpty() && lib.state is FfiLoad.Failed -> item(key = "err") {
                ErrorState("Не удалось загрузить Мою музыку", (lib.state as FfiLoad.Failed).message) { core.loadLibrary(true) }
            }
            lib.tracks.isEmpty() -> item(key = "empty") {
                EmptyState(VIcons.Heart, "В Моей музыке пока пусто", "Ставьте лайки трекам или загрузите свои аудиофайлы")
            }
            else -> {
                trackItems(
                    list = lib.tracks,
                    now = now,
                    label = "Моя музыка",
                    keyPrefix = "lib",
                    showLiked = false,
                    onPlay = { i -> core.playLibrary(i.toUInt()) },
                )
                if (lib.loadingMore) {
                    item(key = "more-spin") {
                        Box(Modifier.fillMaxWidth().padding(vertical = 18.dp), contentAlignment = Alignment.Center) {
                            Spinner(20.dp, c.ink2)
                        }
                    }
                } else if (lib.moreFailed) {
                    item(key = "more-err") {
                        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                            Pill("Не удалось подгрузить, повторить") { core.loadMoreLibrary() }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ColumnScope.LibraryMenuSheet(totalTracks: Long) {
    val app = LocalApp.current
    SheetTitle("Моя музыка")
    MenuRow(VIcons.Upload, "Загрузить аудио", "Отправить MP3 в Мою музыку") {
        app.overlay.close()
        app.uploadOpen = true
    }
    MenuRow(VIcons.Download, "Скачать медиатеку", "Закрепить треки офлайн") {
        app.overlay.open { BulkDownloadSheet(totalTracks) }
    }
}

@Composable
fun ColumnScope.BulkDownloadSheet(totalTracks: Long) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    var picked by remember { mutableStateOf<UInt?>(100u) }
    val choices = listOf(
        50u to "Первые 50",
        100u to "Первые 100",
        500u to "Первые 500",
        null to "Всю медиатеку",
    )
    SheetTitle("Скачать медиатеку", "Треки будут закреплены офлайн и не удалятся лимитом кэша")
    Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Wrap(spacing = 8.dp) {
            choices.forEach { (n, label) ->
                MoodChip(label, picked == n) { picked = n }
            }
        }
        val note = if (picked == null && totalTracks > 0) {
            "Все ${formatTracksCount(totalTracks)} будут закреплены офлайн"
        } else if (picked != null) {
            "Будут скачаны первые $picked треков из Моей музыки"
        } else {
            "Все треки Моей музыки будут закреплены офлайн"
        }
        VText(note, color = c.ink2, size = 13.5.sp)
        Spacer(Modifier.height(4.dp))
        PrimaryPill(VIcons.Download, "Скачать", Modifier.fillMaxWidth()) {
            app.overlay.close()
            app.core.downloadLibrary(picked)
        }
    }
}
