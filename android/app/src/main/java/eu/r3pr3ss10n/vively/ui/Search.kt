package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiLoad

@Composable
fun SearchScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val search = snapshotOf(app.store.searchRev) { core.search() }
    val now = rememberNowPlaying()

    var input by rememberSaveable { mutableStateOf(search?.query ?: "") }
    LaunchedEffect(search?.query) {
        val sq = search?.query
        if (sq != null && sq != input) input = sq
    }

    val q = (search?.query ?: input).trim()

    Column(Modifier.fillMaxSize()) {
        PageTitle("Поиск", if (q.isEmpty()) "Треки, альбомы, плейлисты и исполнители" else null) {
            if (q.isNotEmpty() && search?.state is FfiLoad.Loading) {
                Spinner(18.dp, c.ink2)
            }
        }
        VField(
            value = input,
            onChange = {
                input = it
                core.editSearch(it)
            },
            placeholder = "Поиск треков, альбомов, исполнителей",
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
            leading = VIcons.Search,
            height = 48.dp,
            imeAction = ImeAction.Search,
            onDone = { core.commitSearch(input) },
            onClear = {
                input = ""
                core.clearSearch()
            },
        )

        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(bottom = dockPad()),
        ) {
            if (q.isEmpty()) {
                val hist = search?.history.orEmpty()
                if (hist.isEmpty()) {
                    item(key = "empty-hist") {
                        EmptyState(
                            VIcons.Search,
                            "Найдите любимую музыку",
                            "Введите название трека, альбома, плейлиста или имя исполнителя в строке поиска выше",
                        )
                    }
                } else {
                    item(key = "hist-h") {
                        SectionHeader("Недавние запросы", action = "Очистить", topPadding = 12.dp, onAction = { core.clearSearchHistory() })
                    }
                    item(key = "hist-chips") {
                        Wrap(Modifier.padding(horizontal = 16.dp), spacing = 8.dp) {
                            hist.forEach { entry ->
                                Row(
                                    Modifier.height(38.dp).clip(CircleShape).background(c.fill).padding(start = 16.dp, end = 4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                                ) {
                                    VText(
                                        entry,
                                        Modifier.tap {
                                            input = entry
                                            core.pickSearchHistory(entry)
                                        }.padding(vertical = 4.dp),
                                        size = 14.sp,
                                        weight = FontWeight.Medium,
                                        maxLines = 1,
                                    )
                                    IconBtn(
                                        VIcons.Close,
                                        { core.deleteSearchHistory(entry) },
                                        size = 26.dp,
                                        iconSize = 13.dp,
                                    )
                                }
                            }
                        }
                    }
                }
            } else {
                val s = search
                val empty = s == null || (s.tracks.isEmpty() && s.artists.isEmpty() && s.playlists.isEmpty() && s.sections.isEmpty())
                when {
                    s == null || (empty && (s.state is FfiLoad.Loading || s.state is FfiLoad.Idle)) -> {
                        item(key = "skel") {
                            Column {
                                ShelfSkeleton()
                                TrackListSkeleton(6)
                            }
                        }
                    }
                    empty && s.state is FfiLoad.Failed -> {
                        item(key = "err") {
                            ErrorState("Не удалось выполнить поиск", (s.state as FfiLoad.Failed).message) { core.commitSearch(q) }
                        }
                    }
                    empty -> {
                        item(key = "none") {
                            EmptyState(VIcons.Search, "Ничего не найдено", "По запросу «$q» совпадений нет")
                        }
                    }
                    else -> {
                        val hasOtherBlocks = s.artists.isNotEmpty() || s.playlists.isNotEmpty() || s.sections.isNotEmpty()
                        var firstSec = true
                        fun nextTop() = if (firstSec) { firstSec = false; 12.dp } else 24.dp
                        if (s.tracks.isNotEmpty()) {
                            val top = nextTop()
                            item(key = "tr-h") { SectionHeader("Треки", topPadding = top) }
                            if (hasOtherBlocks && s.tracks.size > 3) {
                                item(key = "tr-tc") {
                                    TrackColumnsShelf(s.tracks.take(18), now, label = "Поиск: $q")
                                }
                            } else {
                                trackItems(s.tracks, now, label = "Поиск: $q", keyPrefix = "sr-t")
                            }
                        }
                        if (s.artists.isNotEmpty()) {
                            val top = nextTop()
                            item(key = "ar-h") { SectionHeader("Исполнители", topPadding = top) }
                            item(key = "ar-s") { ArtistShelf(s.artists) }
                        }
                        if (s.playlists.isNotEmpty()) {
                            val top = nextTop()
                            item(key = "pl-h") { SectionHeader("Альбомы и плейлисты", topPadding = top) }
                            item(key = "pl-s") { PlaylistShelf(s.playlists) }
                        }
                        s.sections.forEachIndexed { i, sec ->
                            val top = nextTop()
                            sectionItems(sec, now, "sr-sec-$i", maxTracks = 15, useColumns = sec.columns || sec.tracks.size > 3, topPadding = top)
                        }
                        item(key = "pad") { Spacer(Modifier.height(12.dp)) }
                    }
                }
            }
        }
    }
}
