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
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
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
import androidx.compose.ui.draw.alpha
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiFriend
import eu.r3pr3ss10n.vively.core.FfiLoad
import eu.r3pr3ss10n.vively.core.formatFriendsCount
import eu.r3pr3ss10n.vively.core.formatPlaylistsCount
import eu.r3pr3ss10n.vively.core.formatTracksCount

@Composable
fun ArtistScreen(id: String, name: String) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    LaunchedEffect(id) { core.openArtist(id) }

    val artist = snapshotOf(app.store.pagesRev, id) { core.artist(id) }
    val now = rememberNowPlaying()
    val list = rememberLazyListState()

    val title = artist?.name?.takeIf { it.isNotBlank() && it != id } ?: name.ifBlank { "Исполнитель" }
    val artistTint = rememberArtworkTint(artist?.photoUrl, null)
    val loadingInitial = artist == null || (
        (artist.state is FfiLoad.Idle || artist.state is FfiLoad.Loading) &&
            artist.tracks.isEmpty() && artist.sections.isEmpty() && artist.playlists.isEmpty()
    )

    val heroTop = c.heroTop(artistTint)
    val fade = headerFade(list, 180.dp, 280.dp)
    val tracks = artist?.tracks.orEmpty()

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
                        .padding(top = 48.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(Modifier.shadow(24.dp, CircleShape)) { Cover(artist?.photoUrl, 176.dp, round = true) }
                    Spacer(Modifier.height(18.dp))
                    VText(
                        title,
                        Modifier.padding(horizontal = 16.dp),
                        size = 23.sp,
                        weight = FontWeight.Bold,
                        align = TextAlign.Center,
                        maxLines = 2,
                        lineHeight = 27.sp,
                        tracking = (-0.3).sp,
                    )
                    VText("Исполнитель", Modifier.padding(top = 4.dp), color = c.ink2, size = 13.5.sp, weight = FontWeight.Medium)
                    Spacer(Modifier.height(12.dp))
                    CollectionActions(
                        onPlay = if (tracks.isNotEmpty()) { { core.playTracks(tracks.map { it.fullId }, 0u, title) } } else null,
                        onShuffle = if (tracks.isNotEmpty()) { { core.playTracksShuffled(tracks.map { it.fullId }, title) } } else null,
                    ) {
                        if (artist != null) {
                            Pill(
                                if (artist.followed) "Вы подписаны" else "Подписаться",
                                Modifier.padding(start = 10.dp, end = 4.dp),
                                icon = if (artist.followed) VIcons.Check else VIcons.Plus,
                                height = 36.dp,
                            ) { core.toggleArtistFollow(id) }
                        }
                        ActionIcon(VIcons.Wave, { core.startArtistMix(id, title) })
                    }
                    Spacer(Modifier.height(4.dp))
                }
            }

            when {
                loadingInitial -> item(key = "skel") {
                    Column {
                        TrackListSkeleton(6)
                        ShelfSkeleton()
                    }
                }
                artist.tracks.isEmpty() && artist.sections.isEmpty() && artist.playlists.isEmpty() && artist.state is FfiLoad.Failed -> {
                    item(key = "err") {
                        ErrorState("Не удалось загрузить исполнителя", (artist.state as FfiLoad.Failed).message) { core.reloadArtist(id) }
                    }
                }
                else -> {
                    if (artist.sections.isNotEmpty()) {
                        artist.sections.forEachIndexed { i, sec ->
                            sectionItems(sec, now, "ar-s-$id-$i", maxTracks = 15, useColumns = true, topPadding = if (i == 0) 12.dp else 24.dp)
                        }
                    } else {
                        if (artist.tracks.isNotEmpty()) {
                            item(key = "pop-h") { SectionHeader("Популярные треки", topPadding = 12.dp) }
                            item(key = "pop-tc") { TrackColumnsShelf(artist.tracks.take(15), now, label = title) }
                        }
                        if (artist.playlists.isNotEmpty()) {
                            item(key = "pl-h") { SectionHeader("Релизы и плейлисты") }
                            item(key = "pl-s") { PlaylistShelf(artist.playlists) }
                        }
                    }
                    if (artist.related.isNotEmpty()) {
                        item(key = "rel-h") { SectionHeader("Похожие исполнители") }
                        item(key = "rel-s") { ArtistShelf(artist.related) }
                    }
                }
            }
        }
        FadingTopBar(title, fade, lerp(heroTop, c.bg, 0.35f), onBack = { app.nav.pop() })
    }
}

@Composable
fun FriendsScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    LaunchedEffect(Unit) { core.loadFriends(false) }

    val friends = snapshotOf(app.store.friendsRev) { core.friends() }
    val q = app.filters["friends"] ?: ""
    val items = friends?.items.orEmpty()
    val filtered = remember(items, q) {
        val needle = q.trim().lowercase()
        if (needle.isEmpty()) items else items.filter { it.name.lowercase().contains(needle) }
    }
    val total = maxOf(friends?.total ?: 0L, items.size.toLong())
    val loadingInitial = friends == null || ((friends.state is FfiLoad.Idle || friends.state is FfiLoad.Loading) && items.isEmpty())
    val title = if (!loadingInitial && total > 0) "Друзья · ${formatFriendsCount(total)}" else "Друзья"

    Column(Modifier.fillMaxSize()) {
        TopBar(title, titleVisible = true, onBack = { app.nav.pop() })
        if (items.size > 6) {
            VField(
                value = q,
                onChange = { app.filters["friends"] = it },
                placeholder = "Поиск среди друзей",
                modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 8.dp),
                leading = VIcons.Search,
                height = 40.dp,
                onClear = { app.filters["friends"] = "" },
            )
        }

        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(top = 4.dp, bottom = dockPad()),
        ) {
            when {
                loadingInitial -> item(key = "skel") { TrackListSkeleton(8) }
                items.isEmpty() && friends.state is FfiLoad.Failed -> item(key = "err") {
                    ErrorState("Не удалось загрузить список друзей", (friends.state as FfiLoad.Failed).message) { core.loadFriends(true) }
                }
                items.isEmpty() -> item(key = "empty") {
                    EmptyState(
                        VIcons.Users,
                        "Список друзей пуст",
                        "Когда появятся друзья ВКонтакте, их музыка и плейлисты будут доступны здесь",
                    )
                }
                filtered.isEmpty() -> item(key = "nomatch") {
                    EmptyState(VIcons.Search, "Ничего не найдено", "Нет друзей с именем «$q»")
                }
                else -> {
                    items(filtered, key = { it.id }) { f ->
                        FriendRow(f) { app.openFriend(f.id) }
                    }
                }
            }
        }
    }
}

@Composable
private fun FriendRow(friend: FfiFriend, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .height(64.dp)
            .tap(shape = Square, onClick = onClick)
            .alpha(if (friend.closed) 0.5f else 1f)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Avatar(friend.photoUrl, friend.id, friend.name, 46.dp)
        Column(Modifier.weight(1f)) {
            VText(friend.name, color = c.ink, size = 15.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
            if (friend.closed) {
                VText("Доступ к аудио ограничен", color = c.ink2, size = 13.5.sp, maxLines = 1)
            }
        }
        VIcon(if (friend.closed) VIcons.Lock else VIcons.ChevRight, 17.dp, c.ink3)
    }
}

@Composable
fun FriendScreen(id: Long) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val auth by app.store.auth.collectAsState()
    LaunchedEffect(id) { core.openFriend(id, false) }

    val fm = snapshotOf(app.store.friendsRev, id) { core.friendMusic(id) }
    val ps = snapshotOf(app.store.friendsRev, id) { core.profileSearch() }?.takeIf { it.ownerId == id }
    val now = rememberNowPlaying()
    val list = rememberLazyListState()
    var searchQuery by rememberSaveable(id) { mutableStateOf("") }

    if (fm == null) {
        Column(Modifier.fillMaxSize()) {
            TopBar("", false, onBack = { app.nav.pop() })
            TrackListSkeleton(8)
        }
        return
    }

    val friend = fm.friend
    val friendTint = rememberArtworkTint(friend.photoUrl, null)
    val loadingInitial = (fm.state is FfiLoad.Idle || fm.state is FfiLoad.Loading) && fm.tracks.isEmpty() && fm.playlists.isEmpty()
    val hasMoreTracks = fm.tracks.size.toLong() < fm.totalTracks
    val hasMorePlaylists = fm.playlists.size.toLong() < fm.totalPlaylists
    val searching = searchQuery.trim().isNotEmpty()

    NearEndEffect(list) {
        if (!searching && hasMoreTracks && !fm.loadingMore) core.loadMoreFriendTracks(id)
    }

    val heroTop = c.heroTop(friendTint)
    val fade = headerFade(list, 150.dp, 250.dp)

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
                        .padding(top = 48.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    Box(Modifier.shadow(20.dp, CircleShape)) { Avatar(friend.photoUrl, friend.id, friend.name, 132.dp) }
                    Spacer(Modifier.height(16.dp))
                    VText(
                        friend.name,
                        Modifier.padding(horizontal = 16.dp),
                        size = 22.sp,
                        weight = FontWeight.Bold,
                        align = TextAlign.Center,
                        maxLines = 2,
                        tracking = (-0.3).sp,
                    )
                    val facts = buildList {
                        if (fm.totalTracks > 0) add(formatTracksCount(fm.totalTracks))
                        if (fm.totalPlaylists > 0) add(formatPlaylistsCount(fm.totalPlaylists))
                    }
                    if (facts.isNotEmpty()) {
                        VText(facts.joinToString(" · "), Modifier.padding(top = 4.dp), color = c.ink2, size = 13.5.sp)
                    }
                    Spacer(Modifier.height(12.dp))
                    CollectionActions(
                        onPlay = if (fm.tracks.isNotEmpty()) { { core.playTracks(fm.tracks.map { it.fullId }, 0u, friend.name) } } else null,
                        onShuffle = if (fm.tracks.isNotEmpty()) { { core.playTracksShuffled(fm.tracks.map { it.fullId }, friend.name) } } else null,
                    ) {
                        if (auth.userId != id) {
                            Pill(
                                if (fm.following) "Вы подписаны" else "Подписаться",
                                Modifier.padding(start = 10.dp),
                                icon = if (fm.following) VIcons.Check else VIcons.Plus,
                                height = 36.dp,
                            ) { core.toggleOwnerFollow(id) }
                        }
                    }
                    Spacer(Modifier.height(8.dp))
                }
            }

            if (!friend.closed) {
                item(key = "psearch") {
                    VField(
                        value = searchQuery,
                        onChange = {
                            searchQuery = it
                            if (it.isBlank()) core.searchInProfile(id, "")
                        },
                        placeholder = "Поиск по музыке профиля",
                        modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
                        leading = VIcons.Search,
                        height = 40.dp,
                        imeAction = ImeAction.Search,
                        onDone = { core.searchInProfile(id, searchQuery) },
                        onClear = {
                            searchQuery = ""
                            core.searchInProfile(id, "")
                        },
                    )
                }
            }

            if (searching && ps != null && ps.query.isNotBlank()) {
                when {
                    ps.state is FfiLoad.Loading && ps.tracks.isEmpty() && ps.playlists.isEmpty() -> {
                        item(key = "ps-skel") { TrackListSkeleton(5) }
                    }
                    ps.state is FfiLoad.Failed && ps.tracks.isEmpty() && ps.playlists.isEmpty() -> {
                        item(key = "ps-err") {
                            ErrorState("Ошибка поиска", (ps.state as FfiLoad.Failed).message) { core.searchInProfile(id, searchQuery) }
                        }
                    }
                    ps.tracks.isEmpty() && ps.playlists.isEmpty() -> {
                        item(key = "ps-empty") {
                            EmptyState(VIcons.Search, "Ничего не найдено", "В профиле нет совпадений по «${ps.query}»")
                        }
                    }
                    else -> {
                        if (ps.playlists.isNotEmpty()) {
                            item(key = "ps-pl-h") { SectionHeader("Плейлисты") }
                            item(key = "ps-pl-s") { PlaylistShelf(ps.playlists) }
                        }
                        if (ps.tracks.isNotEmpty()) {
                            item(key = "ps-tr-h") { SectionHeader("Треки") }
                            trackItems(ps.tracks, now, label = friend.name, keyPrefix = "ps-t-$id")
                        }
                    }
                }
            } else {
                when {
                    friend.closed -> item(key = "closed") {
                        EmptyState(VIcons.Lock, "Доступ ограничен", "Аудиозаписи этого профиля скрыты настройками приватности")
                    }
                    loadingInitial -> item(key = "skel") { TrackListSkeleton(8) }
                    fm.tracks.isEmpty() && fm.playlists.isEmpty() && fm.state is FfiLoad.Failed -> item(key = "err") {
                        ErrorState("Не удалось загрузить музыку", (fm.state as FfiLoad.Failed).message) { core.openFriend(id, true) }
                    }
                    fm.tracks.isEmpty() && fm.playlists.isEmpty() -> item(key = "empty") {
                        EmptyState(VIcons.Users, "Нет доступной музыки", "У пользователя нет открытых аудиозаписей и плейлистов")
                    }
                    else -> {
                        if (fm.playlists.isNotEmpty()) {
                            item(key = "pl-h") {
                                SectionHeader(
                                    "Плейлисты",
                                    action = "Загрузить ещё",
                                    onAction = if (hasMorePlaylists) { { core.loadMoreFriendPlaylists(id) } } else null,
                                )
                            }
                            item(key = "pl-s") { PlaylistShelf(fm.playlists) }
                        } else if (!fm.playlistsError.isNullOrBlank()) {
                            item(key = "pl-err") {
                                VText(
                                    "Плейлисты недоступны: ${fm.playlistsError}",
                                    Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                                    color = c.ink2,
                                    size = 13.sp,
                                )
                            }
                        }
                        if (fm.tracks.isNotEmpty()) {
                            item(key = "tr-h") { SectionHeader("Треки") }
                            trackItems(fm.tracks, now, label = friend.name, keyPrefix = "fr-t-$id")
                            if (fm.loadingMore) {
                                item(key = "more") {
                                    Box(Modifier.fillMaxWidth().padding(vertical = 18.dp), contentAlignment = Alignment.Center) {
                                        Spinner(20.dp, c.ink2)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        FadingTopBar(friend.name, fade, lerp(heroTop, c.bg, 0.35f), onBack = { app.nav.pop() })
    }
}

@Composable
fun SectionScreen(key: String) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val page = snapshotOf(app.store.pagesRev, key) { core.section(key) }
    val now = rememberNowPlaying()
    val list = rememberLazyListState()

    NearEndEffect(list) {
        if (page != null && page.canLoadMore && !page.loadingMore) core.loadMoreSection(key)
    }

    val title = page?.title ?: "Раздел"
    val allTracks = remember(page?.sections) { page?.sections.orEmpty().flatMap { it.tracks } }

    val fade = headerFade(list, 40.dp, 90.dp)

    Box(Modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            state = list,
            contentPadding = PaddingValues(bottom = dockPad()),
        ) {
            item(key = "head") {
                Column(Modifier.fillMaxWidth().statusBarsPadding().padding(top = 56.dp)) {
                    VText(
                        title,
                        Modifier.padding(horizontal = 16.dp),
                        size = 24.sp,
                        weight = FontWeight.Bold,
                        lineHeight = 29.sp,
                        tracking = (-0.3).sp,
                    )
                    if (allTracks.isNotEmpty()) {
                        Spacer(Modifier.height(8.dp))
                        CollectionActions(
                            onPlay = { core.playTracks(allTracks.map { it.fullId }, 0u, title) },
                            onShuffle = { core.playTracksShuffled(allTracks.map { it.fullId }, title) },
                        )
                    }
                }
            }

            when {
                page == null || (page.sections.isEmpty() && (page.state is FfiLoad.Idle || page.state is FfiLoad.Loading)) -> {
                    item(key = "skel") {
                        Column {
                            ShelfSkeleton()
                            TrackListSkeleton(6)
                        }
                    }
                }
                page.sections.isEmpty() && page.state is FfiLoad.Failed -> {
                    item(key = "err") {
                        ErrorState("Не удалось загрузить раздел", (page.state as FfiLoad.Failed).message) { core.reloadSection(key) }
                    }
                }
                page.sections.isEmpty() -> {
                    item(key = "empty") {
                        EmptyState(VIcons.Album, "Раздел пока пуст", "Здесь пока нет доступных треков и подборок")
                    }
                }
                else -> {
                    page.sections.forEachIndexed { i, sec ->
                        sectionItems(sec, now, "sec-$key-$i", maxTracks = Int.MAX_VALUE, topPadding = if (i == 0) 12.dp else 24.dp)
                    }
                    if (page.loadingMore) {
                        item(key = "more") {
                            Box(Modifier.fillMaxWidth().padding(vertical = 18.dp), contentAlignment = Alignment.Center) {
                                Spinner(20.dp, c.ink2)
                            }
                        }
                    } else if (page.moreFailed) {
                        item(key = "more-err") {
                            Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.Center) {
                                Pill("Повторить загрузку") { core.loadMoreSection(key) }
                            }
                        }
                    }
                }
            }
        }
        FadingTopBar(title, fade, c.bg, onBack = { app.nav.pop() })
    }
}
