package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiDownloadState
import eu.r3pr3ss10n.vively.core.FfiPendingDownload
import eu.r3pr3ss10n.vively.core.formatGb
import eu.r3pr3ss10n.vively.core.formatTracksCount

@Composable
fun DownloadsScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val dl = snapshotOf(app.store.downloadsRev) { core.downloads() }
    val lib by app.store.library.collectAsState()
    val now = rememberNowPlaying()
    val q = app.filters["downloads"] ?: ""

    val ready = dl?.ready.orEmpty()
    val pending = dl?.pending.orEmpty()
    val collections = dl?.collections.orEmpty()
    val filtered = remember(ready, q) {
        val needle = q.trim().lowercase()
        if (needle.isEmpty()) ready
        else ready.filter { it.title.lowercase().contains(needle) || it.artist.lowercase().contains(needle) }
    }
    val completelyEmpty = ready.isEmpty() && collections.isEmpty() && pending.isEmpty()
    val subtitle = if (completelyEmpty || dl == null) {
        "Музыка для прослушивания без интернета"
    } else {
        "${formatTracksCount(ready.size.toLong())} · ${formatGb(dl.totalBytes)}"
    }

    Column(Modifier.fillMaxSize()) {
        TopBar(
            title = "Загруженное",
            titleVisible = true,
            onBack = { app.nav.pop() },
            actions = {
                IconBtn(VIcons.Download, { app.overlay.open { BulkDownloadSheet(lib.total) } }, tint = c.ink)
                if (!completelyEmpty) {
                    IconBtn(VIcons.Trash, { app.overlay.open { ConfirmClearDownloadsSheet() } }, tint = c.ink2)
                }
            },
        )
        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(bottom = dockPad()),
        ) {
            if (!completelyEmpty) {
                item(key = "sub") {
                    VText(subtitle, Modifier.padding(horizontal = 16.dp, vertical = 4.dp), color = c.ink2, size = 13.5.sp)
                }
            }

            if (dl == null) {
                item(key = "skel") { TrackListSkeleton(6) }
            } else if (completelyEmpty) {
                item(key = "empty") {
                    EmptyState(
                        VIcons.Download,
                        "Нет загруженной музыки",
                        "Скачивайте треки, плейлисты или всю медиатеку, чтобы слушать их офлайн",
                    ) {
                        PrimaryPill(VIcons.Download, "Скачать медиатеку") {
                            app.overlay.open { BulkDownloadSheet(lib.total) }
                        }
                    }
                }
            } else {
                if (ready.isNotEmpty()) {
                    item(key = "actions") {
                        CollectionActions(
                            onPlay = {
                                if (q.isBlank()) core.playDownloads(0u, false)
                                else if (filtered.isNotEmpty()) core.playTracks(filtered.map { it.fullId }, 0u, "Загруженное")
                            },
                            onShuffle = {
                                if (q.isBlank()) core.playDownloads(0u, true)
                                else if (filtered.isNotEmpty()) core.playTracksShuffled(filtered.map { it.fullId }, "Загруженное")
                            },
                            modifier = Modifier.padding(vertical = 4.dp),
                        )
                    }
                }

                if (pending.isNotEmpty()) {
                    item(key = "pend-h") {
                        SectionHeader(
                            "Загружаются · ${pending.size}",
                            action = "Отменить",
                            topPadding = 16.dp,
                            onAction = { core.cancelPendingDownloads() },
                        )
                    }
                    items(pending, key = { "pd-${it.fullId}" }) { p ->
                        PendingDownloadRow(p)
                    }
                }

                if (collections.isNotEmpty()) {
                    item(key = "col-h") { SectionHeader("Плейлисты и альбомы") }
                    item(key = "col-s") { PlaylistShelf(collections, offline = true) }
                }

                if (ready.isNotEmpty()) {
                    item(key = "tr-h") {
                        Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 24.dp, bottom = 8.dp)) {
                            VText("Треки · ${ready.size}", size = 19.sp, weight = FontWeight.Bold)
                            if (ready.size > 6) {
                                Spacer(Modifier.height(12.dp))
                                VField(
                                    value = q,
                                    onChange = { app.filters["downloads"] = it },
                                    placeholder = "Поиск в загруженном",
                                    leading = VIcons.Search,
                                    height = 40.dp,
                                    onClear = { app.filters["downloads"] = "" },
                                )
                            }
                        }
                    }
                    if (filtered.isEmpty()) {
                        item(key = "nomatch") {
                            EmptyState(VIcons.Search, "Ничего не найдено", "Среди загруженных треков нет совпадений")
                        }
                    } else {
                        trackItems(
                            list = filtered,
                            now = now,
                            label = "Загруженное",
                            keyPrefix = "dl",
                            onPlay = { i ->
                                if (q.isBlank()) core.playDownloads(i.toUInt(), false)
                                else core.playTracks(filtered.map { it.fullId }, i.toUInt(), "Загруженное")
                            },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun PendingDownloadRow(item: FfiPendingDownload) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val pct = item.progress?.toFloat()?.coerceIn(0f, 1f)
    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 6.dp, top = 7.dp, bottom = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Cover(item.artworkUrl, 44.dp)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            VText(item.title, size = 14.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
            VText(item.artist, color = c.ink2, size = 13.sp, maxLines = 1)
            if (pct != null && item.state == FfiDownloadState.DOWNLOADING) {
                Box(Modifier.fillMaxWidth().height(3.dp).clip(RS).background(c.fill2)) {
                    Box(Modifier.fillMaxWidth(pct).fillMaxHeight().background(c.hi))
                }
            }
        }
        when (item.state) {
            FfiDownloadState.DOWNLOADING -> {
                if (pct != null) VText("${(pct * 100).toInt()}%", color = c.ink2, size = 12.sp, weight = FontWeight.SemiBold)
                else Spinner(16.dp, c.hi)
            }
            FfiDownloadState.QUEUED -> VText("В очереди", color = c.ink2, size = 12.5.sp)
            FfiDownloadState.FAILED -> VText("Ошибка", color = c.danger, size = 12.sp, weight = FontWeight.SemiBold)
            FfiDownloadState.READY -> VIcon(VIcons.Downloaded, 18.dp, c.hi)
        }
        IconBtn(VIcons.Close, { app.core.removeDownload(item.fullId) }, size = 34.dp, iconSize = 15.dp)
    }
}

@Composable
fun androidx.compose.foundation.layout.ColumnScope.ConfirmClearDownloadsSheet() {
    val app = LocalApp.current
    SheetTitle("Удалить все загрузки?", "Все сохранённые треки и автокэш будут удалены с устройства.")
    Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Pill("Отмена", Modifier.weight(1f)) { app.overlay.close() }
        Pill("Удалить", Modifier.weight(1f), danger = true) {
            app.overlay.close()
            app.core.clearAllDownloads()
        }
    }
}
