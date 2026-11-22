package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiArtistMix
import eu.r3pr3ss10n.vively.core.FfiHome
import eu.r3pr3ss10n.vively.core.FfiHomeBlock
import eu.r3pr3ss10n.vively.core.FfiLoad
import eu.r3pr3ss10n.vively.core.FfiRecentKind


@Composable
fun HomeScreen() {
    val app = LocalApp.current
    val core = app.core
    LaunchedEffect(Unit) { core.loadHome(false) }
    val home = snapshotOf(app.store.homeRev) { core.home() }
    val now = rememberNowPlaying()
    val list = rememberLazyListState()
    LazyColumn(Modifier.fillMaxSize(), state = list, contentPadding = PaddingValues(bottom = dockPad())) {
        item(key = "top") { HomeTopBar() }
        item(key = "wave") { home?.let { WaveHero(it) } }
        item(key = "recent") { home?.let { QuickLinks(it) } }
        when {
            home == null || (home.blocks.isEmpty() && (home.state is FfiLoad.Loading || home.state is FfiLoad.Idle)) -> {
                item(key = "skel") { Column { ShelfSkeleton(); ShelfSkeleton() } }
            }
            home.blocks.isEmpty() && home.state is FfiLoad.Failed -> item(key = "err") {
                ErrorState("Не удалось загрузить Главную", (home.state as FfiLoad.Failed).message) { core.loadHome(true) }
            }
            else -> home.blocks.forEachIndexed { i, b ->
                when (b) {
                    is FfiHomeBlock.Section -> sectionItems(b.section, now, "hb$i", maxTracks = 15, useColumns = b.section.columns || b.section.tracks.size > 3)
                    is FfiHomeBlock.Recommended -> {
                        item(key = "hb$i-h") {
                            SectionHeader(b.title, onAction = b.showAllId?.let { id -> { app.openSection(id, b.title) } })
                        }
                        item(key = "hb$i-r") {
                            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                                items(b.items, key = { it.playlist.key }) { r ->
                                    PlaylistCard(r.playlist, subtitle = r.matchLabel, dotColor = hexColor(r.colorHex) ?: VivelyTheme.colors.ink2)
                                }
                            }
                        }
                    }
                    is FfiHomeBlock.MixTiles -> {
                        item(key = "hb$i-h") { SectionHeader(b.title) }
                        item(key = "hb$i-t") {
                            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                items(b.tiles, key = { it.id }) { t ->
                                    Pill(t.title, height = 40.dp) { core.startMix(t.mixId, null, t.optionsJson, t.title) }
                                }
                            }
                        }
                    }
                    is FfiHomeBlock.ArtistMixes -> {
                        item(key = "hb$i-h") { SectionHeader(b.title) }
                        item(key = "hb$i-a") {
                            LazyRow(contentPadding = PaddingValues(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                                items(b.cards, key = { it.id }) { ArtistMixCard(it) }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun HomeTopBar() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .statusBarsPadding()
            .padding(start = 16.dp, end = 8.dp, top = 14.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        VIcon(VIcons.Logo, 22.dp, c.hi)
        VText("Vively", Modifier.weight(1f), size = 22.sp, weight = FontWeight.Bold, tracking = (-0.3).sp)
        IconBtn(VIcons.Settings, { app.openSettings() }, size = 44.dp, iconSize = 22.dp, tint = c.ink)
    }
}

@Composable
private fun WaveHero(home: FfiHome) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    Column(
        Modifier
            .padding(horizontal = 16.dp)
            .fillMaxWidth()
            .clip(RoundedCornerShape(20.dp))
            .background(c.wave)
            .padding(vertical = 18.dp),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = 18.dp, end = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                VIcon(VIcons.Wave, 22.dp, c.hi)
                VText(
                    home.mixTitle,
                    Modifier.padding(top = 8.dp),
                    size = 20.sp,
                    weight = FontWeight.Bold,
                    lineHeight = 24.sp,
                    maxLines = 2,
                    tracking = (-0.2).sp,
                )
                VText(
                    "Бесконечный поток треков под ваш вкус",
                    Modifier.padding(top = 4.dp),
                    color = c.ink2,
                    size = 13.5.sp,
                    maxLines = 2,
                )
            }
            PlayFab(
                {
                    if (home.hasPicks && !home.mixActive) core.startTunedMix(home.mixId) else core.startHomeMix()
                },
                size = 52.dp,
                playing = home.mixPlaying,
            )
        }
        Row(
            Modifier.fillMaxWidth().padding(start = 18.dp, end = 18.dp, top = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Pill("Настроить", icon = VIcons.Sliders, height = 36.dp) { app.overlay.open { MixTuneSheet(home.mixId) } }
            if (home.hasPicks) {
                VText(
                    "Сбросить",
                    Modifier.tap(shape = CircleShape) { core.clearMixPicks() }.padding(horizontal = 10.dp, vertical = 8.dp),
                    color = c.ink2, size = 14.sp, weight = FontWeight.SemiBold,
                )
            }
        }
    }
}

@Composable
private fun ColumnScope.MixTuneSheet(mixId: String) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val home = snapshotOf(app.store.homeRev) { core.home() } ?: return
    SheetTitle("Настроить микс")
    Column(
        Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        home.mixCategories.forEach { cat ->
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                VText(cat.title, color = c.ink2, size = 13.sp, weight = FontWeight.SemiBold)
                Wrap { cat.options.forEach { o -> MoodChip(o.title, o.selected) { core.toggleMixOption(cat.id, o.id) } } }
            }
        }
    }
    Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 16.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Pill("Сбросить", enabled = home.hasPicks) { core.clearMixPicks() }
        PrimaryPill(VIcons.Play, "Запустить свой микс", Modifier.weight(1f)) {
            app.overlay.close()
            core.startTunedMix(mixId)
        }
    }
}

private data class QuickTileData(
    val title: String,
    val artworkUrl: String?,
    val isLibrary: Boolean = false,
    val round: Boolean = false,
    val onClick: () -> Unit,
)

@Composable
private fun QuickLinks(home: FfiHome) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val tiles = if (home.links.isNotEmpty()) {
        buildList {
            if (home.links.none { it.isLibrary }) {
                add(QuickTileData("Моя музыка", null, isLibrary = true) { app.openLibrary() })
            }
            home.links.take(5).forEach { l ->
                add(QuickTileData(l.title, l.artworkUrl, isLibrary = l.isLibrary) { app.openLink(l.id) })
            }
        }.take(6)
    } else {
        buildList {
            add(QuickTileData("Моя музыка", null, isLibrary = true) { app.openLibrary() })
            home.recents.take(5).forEach { r ->
                add(QuickTileData(r.title, r.artworkUrl, round = r.kind is FfiRecentKind.Artist) { app.openRecent(r.kind) })
            }
        }
    }
    if (tiles.isEmpty()) return

    val pairs = tiles.chunked(2)
    Column(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 12.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        pairs.forEach { row ->
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { item ->
                    Row(
                        Modifier
                            .weight(1f)
                            .height(56.dp)
                            .clip(RS)
                            .background(c.fill)
                            .tap(shape = RS, pressed = c.fill2, onClick = item.onClick)
                            .padding(end = 10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                    ) {
                        if (item.isLibrary) LikedCover(56.dp, radius = 0.dp) else Cover(item.artworkUrl, 56.dp, round = item.round, radius = 0.dp)
                        VText(item.title, Modifier.weight(1f), size = 13.5.sp, weight = FontWeight.SemiBold, maxLines = 2, lineHeight = 16.sp)
                    }
                }
                if (row.size == 1) Spacer(Modifier.weight(1f))
            }
        }
    }
}

@Composable
private fun ArtistMixCard(m: FfiArtistMix) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    Column(
        Modifier.width(CARD_W).tap(pressed = androidx.compose.ui.graphics.Color.Transparent) { app.core.startMix(m.mixId, m.entityId, null, m.title) },
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Cover(m.photoUrl, CARD_W, round = true)
        Spacer(Modifier.height(10.dp))
        VText(m.title, size = 14.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
        m.description?.let { VText(it, color = c.ink2, size = 13.sp, maxLines = 1) }
    }
}
