package eu.r3pr3ss10n.vively.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.platform.LocalDensity
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiLoad
import eu.r3pr3ss10n.vively.core.FfiPlayback
import eu.r3pr3ss10n.vively.core.FfiRepeat
import eu.r3pr3ss10n.vively.core.FfiTrack
import eu.r3pr3ss10n.vively.core.formatDuration
import kotlinx.coroutines.delay

@Composable
fun PlayerScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val pb by app.store.playback.collectAsState()
    val track = pb.current ?: return

    BackHandler(enabled = app.stageTab != null) { app.stageTab = null }

    var pos by remember(track.fullId) { mutableDoubleStateOf(pb.positionSecs) }
    LaunchedEffect(pb.positionSecs) { pos = pb.positionSecs }
    LaunchedEffect(pb.playing, track.fullId) {
        while (pb.playing) {
            delay(250)
            pos = app.core.positionSecs()
        }
    }

    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    val drag = remember { Animatable(0f) }
    val closePx = with(density) { 140.dp.toPx() }
    val dismiss = Modifier.pointerInput(Unit) {
        detectVerticalDragGestures(
            onDragEnd = {
                if (drag.value > closePx) app.playerOpen = false
                scope.launch { drag.animateTo(0f, tween(220)) }
            },
            onDragCancel = { scope.launch { drag.animateTo(0f, tween(220)) } },
        ) { _, dy -> scope.launch { drag.snapTo((drag.value + dy).coerceAtLeast(0f)) } }
    }
    val top = c.heroTop(c.tintHex)
    val bottom = if (c.isDark) lerp(top, Color.Black, 0.62f) else c.bg

    Column(
        Modifier
            .fillMaxSize()
            .graphicsLayer {
                translationY = drag.value
                if (drag.value > 0f) {
                    shape = RoundedCornerShape(topStart = 24.dp, topEnd = 24.dp)
                    clip = true
                }
            }
            .background(Brush.verticalGradient(listOf(top, bottom)))
            .pointerInput(Unit) { detectTapGestures { } }
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        Row(
            Modifier
                .fillMaxWidth()
                .height(56.dp)
                .then(dismiss)
                .padding(horizontal = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            IconBtn(VIcons.ChevDown, { app.playerOpen = false }, size = 44.dp, iconSize = 26.dp, tint = c.ink)
            Column(Modifier.weight(1f).padding(horizontal = 4.dp), horizontalAlignment = Alignment.CenterHorizontally) {
                val albumTitle = track.albumTitle?.trim()?.takeIf { it.isNotEmpty() && !it.equals("unnamed", ignoreCase = true) }
                val heading = when (app.stageTab) {
                    StageTab.Lyrics -> "Текст"
                    StageTab.Queue -> "Очередь"
                    null -> albumTitle ?: pb.playingFrom.ifBlank { "Сейчас играет" }
                }
                val headMod = if (app.stageTab == null && albumTitle != null) {
                    Modifier.tap(shape = CircleShape) { app.openAlbum(track.fullId) }.padding(horizontal = 10.dp, vertical = 4.dp)
                } else {
                    Modifier
                }
                if (app.stageTab == null) {
                    VText("Сейчас играет", color = c.ink2, size = 11.sp, weight = FontWeight.SemiBold, maxLines = 1, tracking = 0.6.sp)
                }
                VText(heading, headMod, color = c.ink, size = 13.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
            }
            IconBtn(VIcons.More, { app.overlay.open { TrackSheet(track, null) } }, size = 44.dp, iconSize = 22.dp, tint = c.ink)
        }

        when (app.stageTab) {
            null -> PlayerArtworkPane(track, pb, pos, Modifier.weight(1f).then(dismiss))
            StageTab.Lyrics -> PlayerLyricsPane(track, pb, Modifier.weight(1f))
            StageTab.Queue -> PlayerQueuePane(track, pb, Modifier.weight(1f))
        }

        PlayerBottomBar(track)
    }
}

@Composable
private fun PlayerBottomBar(track: FfiTrack) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val lyricsOn = app.stageTab == StageTab.Lyrics
    val queueOn = app.stageTab == StageTab.Queue

    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 20.dp, vertical = 10.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconBtn(
            VIcons.Lyrics,
            { app.stageTab = if (lyricsOn) null else StageTab.Lyrics },
            size = 44.dp,
            iconSize = 21.dp,
            tint = if (lyricsOn) c.hi else c.ink2,
            active = lyricsOn,
        )
        IconBtn(
            VIcons.Wave,
            { core.startTrackMix(track.fullId) },
            size = 44.dp,
            iconSize = 21.dp,
            tint = c.ink2,
        )
        if (track.downloadPending) {
            Box(
                Modifier.size(44.dp).clip(RS).tap { core.removeDownload(track.fullId) },
                contentAlignment = Alignment.Center,
            ) { Spinner(19.dp, c.hi) }
        } else {
            IconBtn(
                if (track.downloaded) VIcons.Downloaded else VIcons.Download,
                { if (track.downloaded) core.removeDownload(track.fullId) else core.downloadTrack(track.fullId) },
                size = 44.dp,
                iconSize = 21.dp,
                tint = if (track.downloaded) c.hi else c.ink2,
                enabled = track.playable,
            )
        }
        IconBtn(
            VIcons.Queue,
            { app.stageTab = if (queueOn) null else StageTab.Queue },
            size = 44.dp,
            iconSize = 21.dp,
            tint = if (queueOn) c.hi else c.ink2,
            active = queueOn,
        )
    }
}

@Composable
private fun PlayerArtworkPane(track: FfiTrack, pb: FfiPlayback, pos: Double, modifier: Modifier) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val density = LocalDensity.current
    val scope = rememberCoroutineScope()
    var dragFrac by remember(track.fullId) { mutableStateOf<Float?>(null) }
    val dur = pb.durationSecs.coerceAtLeast(track.durationSecs.toDouble()).coerceAtLeast(0.0)
    val frac = if (dur > 0) (pos / dur).toFloat().coerceIn(0f, 1f) else 0f
    val shownPos = dragFrac?.let { it * dur } ?: pos
    val artScale by animateFloatAsState(if (pb.playing || pb.loading) 1f else 0.86f, tween(360), label = "art")
    val swipe = remember { Animatable(0f) }
    val hasNext by rememberUpdatedState(pb.hasNext)
    val skipPx = with(density) { 90.dp.toPx() }

    Column(
        modifier.fillMaxWidth().padding(horizontal = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Box(
            Modifier
                .weight(1f)
                .fillMaxWidth()
                .padding(vertical = 12.dp)
                .pointerInput(Unit) {
                    detectHorizontalDragGestures(
                        onDragEnd = {
                            val v = swipe.value
                            when {
                                v < -skipPx && hasNext -> core.nextTrack()
                                v > skipPx -> core.prevTrack()
                            }
                            scope.launch { swipe.animateTo(0f, tween(240)) }
                        },
                        onDragCancel = { scope.launch { swipe.animateTo(0f, tween(240)) } },
                    ) { _, dx -> scope.launch { swipe.snapTo(swipe.value + dx * 0.7f) } }
                },
            contentAlignment = Alignment.Center,
        ) {
            Box(
                Modifier
                    .widthIn(max = 420.dp)
                    .fillMaxWidth()
                    .aspectRatio(1f)
                    .graphicsLayer {
                        scaleX = artScale
                        scaleY = artScale
                        translationX = swipe.value
                        rotationZ = swipe.value / 90f
                    }
                    .shadow(if (pb.playing) 28.dp else 12.dp, RoundedCornerShape(12.dp)),
            ) {
                Cover(track.artworkUrl ?: track.thumbUrl, 420.dp, Modifier.fillMaxSize(), radius = 12.dp)
            }
        }

        Column(Modifier.fillMaxWidth().widthIn(max = 480.dp).padding(top = 8.dp, bottom = 4.dp)) {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Column(Modifier.weight(1f)) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        val titleMod = if (track.albumTitle != null) {
                            Modifier.weight(1f, fill = false).tap { app.openAlbum(track.fullId) }
                        } else {
                            Modifier.weight(1f, fill = false)
                        }
                        VText(track.title, titleMod, size = 21.sp, weight = FontWeight.Bold, maxLines = 1, tracking = (-0.2).sp)
                        if (track.explicit) ExplicitBadge()
                    }
                    val artistMod = if (track.artistId != null) {
                        Modifier.padding(top = 2.dp).tap { app.openArtist(track.artistId!!, track.artistName ?: track.artist) }
                    } else {
                        Modifier.padding(top = 2.dp)
                    }
                    VText(track.artist, artistMod, color = c.ink2, size = 16.sp, weight = FontWeight.Medium, maxLines = 1)
                }
                IconBtn(
                    if (track.liked) VIcons.HeartFill else VIcons.Heart,
                    { core.toggleLike(track.fullId) },
                    size = 48.dp,
                    iconSize = 26.dp,
                    tint = c.ink,
                )
            }

            Spacer(Modifier.height(16.dp))

            VSlider(
                value = frac,
                onCommit = { f ->
                    dragFrac = null
                    if (dur > 0) core.seekTo(f * dur)
                },
                onDrag = { dragFrac = it },
                modifier = Modifier.fillMaxWidth(),
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                VText(formatDuration(shownPos), color = c.ink2, size = 12.sp, weight = FontWeight.Medium)
                VText("-" + formatDuration((dur - shownPos).coerceAtLeast(0.0)), color = c.ink2, size = 12.sp, weight = FontWeight.Medium)
            }

            Spacer(Modifier.height(10.dp))

            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconBtn(
                    VIcons.Shuffle,
                    { core.toggleShuffle() },
                    size = 48.dp,
                    iconSize = 22.dp,
                    tint = if (pb.shuffled) c.hi else c.ink2,
                    active = pb.shuffled,
                )
                IconBtn(VIcons.Prev, { core.prevTrack() }, size = 60.dp, iconSize = 32.dp, tint = c.ink)
                Box(
                    Modifier
                        .size(76.dp)
                        .clip(CircleShape)
                        .background(c.ink)
                        .tap(shape = CircleShape, pressed = c.ink2) { core.togglePlayPause() },
                    contentAlignment = Alignment.Center,
                ) {
                    if (pb.loading) Spinner(28.dp, c.bg) else VIcon(if (pb.playing) VIcons.Pause else VIcons.Play, 34.dp, c.bg)
                }
                IconBtn(VIcons.Next, { core.nextTrack() }, enabled = pb.hasNext, size = 60.dp, iconSize = 32.dp, tint = c.ink)
                Box(contentAlignment = Alignment.Center) {
                    IconBtn(
                        VIcons.Repeat,
                        { core.cycleRepeat() },
                        size = 48.dp,
                        iconSize = 22.dp,
                        tint = if (pb.repeat != FfiRepeat.OFF) c.hi else c.ink2,
                        active = pb.repeat != FfiRepeat.OFF,
                    )
                    if (pb.repeat == FfiRepeat.ONE) {
                        Box(
                            Modifier
                                .align(Alignment.TopEnd)
                                .padding(top = 6.dp, end = 6.dp)
                                .size(14.dp)
                                .clip(CircleShape)
                                .background(c.hi),
                            contentAlignment = Alignment.Center,
                        ) {
                            VText("1", color = c.bg, size = 9.sp, weight = FontWeight.Bold)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun CompactPlayerStrip(track: FfiTrack, pb: FfiPlayback) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp)
            .clip(RL)
            .background(c.fill)
            .tap(shape = RL) { app.stageTab = null }
            .padding(horizontal = 10.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Cover(track.thumbUrl, 40.dp)
        Column(Modifier.weight(1f)) {
            VText(track.title, size = 14.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
            VText(track.artist, color = c.ink2, size = 12.5.sp, maxLines = 1)
        }
        IconBtn(
            if (track.liked) VIcons.HeartFill else VIcons.Heart,
            { app.core.toggleLike(track.fullId) },
            size = 36.dp,
            iconSize = 18.dp,
            tint = if (track.liked) c.ink else c.ink2,
        )
        Box(
            Modifier
                .size(38.dp)
                .clip(CircleShape)
                .background(c.ink)
                .tap(shape = CircleShape, pressed = c.ink2) { app.core.togglePlayPause() },
            contentAlignment = Alignment.Center,
        ) {
            if (pb.loading) Spinner(16.dp, c.bg) else VIcon(if (pb.playing) VIcons.Pause else VIcons.Play, 18.dp, c.bg)
        }
    }
}

@Composable
private fun PlayerLyricsPane(track: FfiTrack, pb: FfiPlayback, modifier: Modifier) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val lyrics = snapshotOf(app.store.lyricsRev, track.fullId) { app.core.lyrics() }
    val list = rememberLazyListState()
    var lastManualScroll by remember { mutableLongStateOf(0L) }

    LaunchedEffect(list.isScrollInProgress) {
        if (list.isScrollInProgress) lastManualScroll = System.currentTimeMillis()
    }

    val activeIdx = lyrics?.active?.toInt()
    LaunchedEffect(activeIdx, lyrics?.synced) {
        if (lyrics?.synced == true && activeIdx != null && activeIdx >= 0) {
            if (System.currentTimeMillis() - lastManualScroll > 3500L) {
                runCatching { list.animateScrollToItem((activeIdx - 2).coerceAtLeast(0)) }
            }
        }
    }

    Column(modifier.fillMaxWidth()) {
        CompactPlayerStrip(track, pb)
        when {
            lyrics == null || (lyrics.lines.isEmpty() && (lyrics.state is FfiLoad.Loading || lyrics.state is FfiLoad.Idle)) -> {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    Spinner(26.dp, c.ink2)
                }
            }
            lyrics.lines.isEmpty() && lyrics.state is FfiLoad.Failed -> {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    EmptyState(VIcons.Lyrics, "Текст не найден", (lyrics.state as FfiLoad.Failed).message)
                }
            }
            lyrics.lines.isEmpty() -> {
                Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                    EmptyState(VIcons.Lyrics, "Нет текста песни", "Для этого трека текст пока недоступен")
                }
            }
            else -> {
                Box(Modifier.weight(1f).fillMaxWidth()) {
                    LazyColumn(
                        Modifier.fillMaxSize().fadingEdges(28.dp),
                        state = list,
                        contentPadding = PaddingValues(horizontal = 24.dp, vertical = 28.dp),
                        verticalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        itemsIndexed(lyrics.lines, key = { i, _ -> "ly-$i" }) { i, line ->
                            val isActive = lyrics.synced && activeIdx == i
                            val isPast = lyrics.synced && activeIdx != null && i < activeIdx
                            val text = line.text
                            if (text.isNullOrBlank()) {
                                VText(
                                    "•••",
                                    color = if (isActive) c.ink else c.ink3,
                                    size = 18.sp,
                                    weight = FontWeight.Bold,
                                )
                            } else {
                                val mod = if (lyrics.synced && line.beginMs >= 0) {
                                    Modifier.fillMaxWidth().tap { app.core.seekTo(line.beginMs / 1000.0) }.padding(vertical = 3.dp)
                                } else {
                                    Modifier.fillMaxWidth().padding(vertical = 2.dp)
                                }
                                VText(
                                    text = text,
                                    modifier = mod,
                                    color = when {
                                        !lyrics.synced -> c.ink
                                        isActive -> c.ink
                                        isPast -> c.ink2
                                        else -> c.ink3
                                    },
                                    size = if (lyrics.synced) 26.sp else 18.sp,
                                    weight = FontWeight.Bold,
                                    lineHeight = if (lyrics.synced) 33.sp else 26.sp,
                                    tracking = if (lyrics.synced) (-0.3).sp else TextUnit.Unspecified,
                                )
                            }
                        }
                        lyrics.source?.let { src ->
                            item(key = "src") {
                                VText(
                                    "Источник: $src",
                                    Modifier.padding(top = 12.dp),
                                    color = c.ink3,
                                    size = 12.5.sp,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun PlayerQueuePane(track: FfiTrack, pb: FfiPlayback, modifier: Modifier) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val now = NowPlaying(track.fullId, pb.playing, pb.loading)

    LazyColumn(
        modifier.fillMaxWidth(),
        contentPadding = PaddingValues(top = 4.dp, bottom = 16.dp),
    ) {
        item(key = "now-h") {
            VText("Сейчас играет", Modifier.padding(horizontal = 16.dp, vertical = 8.dp), color = c.ink2, size = 13.sp, weight = FontWeight.SemiBold)
        }
        item(key = "now-t") {
            TrackRow(track, now, onPlay = { core.togglePlayPause() })
        }
        item(key = "up-h") {
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                VText(
                    if (pb.upcoming.isEmpty()) "Далее" else "Далее · ${pb.upcoming.size}",
                    Modifier.weight(1f),
                    color = c.ink2,
                    size = 13.sp,
                    weight = FontWeight.SemiBold,
                )
                if (pb.upcoming.isNotEmpty()) {
                    Pill("В плейлист", icon = VIcons.Plus, height = 32.dp) {
                        app.openEditor(EditorMode(null), fromQueue = true)
                    }
                    Pill("Очистить", height = 32.dp) { core.clearUpcoming() }
                }
            }
        }
        if (pb.upcoming.isEmpty()) {
            item(key = "up-empty") {
                EmptyState(VIcons.Queue, "Очередь пуста", "Добавьте треки через меню «Следующим» или запустите плейлист")
            }
        } else {
            itemsIndexed(pb.upcoming, key = { i, t -> "q-$i-${t.fullId}" }) { i, t ->
                Row(
                    Modifier
                        .fillMaxWidth()
                        .height(64.dp)
                        .tap(shape = Square, onLongClick = { app.overlay.open { TrackSheet(t, null) } }) {
                            core.selectQueueTrack(t.fullId)
                        }
                        .padding(start = 16.dp, end = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Cover(t.thumbUrl, 48.dp)
                    Column(Modifier.weight(1f)) {
                        VText(t.title, size = 15.5.sp, weight = FontWeight.Medium, maxLines = 1)
                        VText(t.artist, color = c.ink2, size = 13.sp, maxLines = 1)
                    }
                    if (i > 0) {
                        IconBtn(VIcons.ChevUp, { core.moveUpcoming(i.toUInt(), (i - 1).toUInt()) }, size = 32.dp, iconSize = 16.dp)
                    }
                    if (i < pb.upcoming.lastIndex) {
                        IconBtn(VIcons.ChevDown, { core.moveUpcoming(i.toUInt(), (i + 1).toUInt()) }, size = 32.dp, iconSize = 16.dp)
                    }
                    IconBtn(VIcons.Close, { core.removeUpcoming(i.toUInt()) }, size = 32.dp, iconSize = 15.dp)
                }
            }
        }
    }
}
