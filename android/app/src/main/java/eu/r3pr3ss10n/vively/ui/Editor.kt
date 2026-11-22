package eu.r3pr3ss10n.vively.ui

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.FfiEditor
import eu.r3pr3ss10n.vively.core.FfiLoad
import eu.r3pr3ss10n.vively.core.FfiTrack
import eu.r3pr3ss10n.vively.core.FfiUploadStatus
import eu.r3pr3ss10n.vively.core.formatTracksCount
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.ByteArrayOutputStream
import java.io.File

private enum class CreatorTab { Details, Tracks }
private enum class PickerSource { Library, Playlists, Search }

@Composable
fun EditorScreen(mode: EditorMode) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val context = LocalContext.current
    val ed = snapshotOf(app.store.editorRev, mode) { core.editor() }
    val saving = ed?.saving == true

    BackHandler(enabled = true) {
        if (!saving) app.editor = null
    }

    val existingPlaylist = remember(mode.editingKey) {
        mode.editingKey?.let { key ->
            core.playlistDetail(key)?.playlist ?: core.playlists().items.find { it.key == key }
        }
    }

    var title by rememberSaveable(mode) { mutableStateOf(existingPlaylist?.title ?: "") }
    var description by rememberSaveable(mode) { mutableStateOf(existingPlaylist?.description ?: "") }
    var creatorTab by rememberSaveable(mode) { mutableStateOf(CreatorTab.Details) }

    val coverPicker = rememberLauncherForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        if (uri != null) {
            app.store.scope.launch(Dispatchers.IO) {
                encodeCoverJpeg(context, uri)?.let { (bytes, previewPath) ->
                    core.editorSetCover(bytes, previewPath)
                }
            }
        }
    }

    val isAddOnly = mode.addTo != null
    val isEdit = mode.editingKey != null
    val draft = ed?.draft.orEmpty()
    val draftIds = remember(draft) { draft.map { it.fullId }.toSet() }

    Column(
        Modifier
            .fillMaxSize()
            .background(c.bg)
            .statusBarsPadding()
            .navigationBarsPadding()
            .imePadding(),
    ) {
        // Header
        Row(
            Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            IconBtn(VIcons.Close, { if (!saving) app.editor = null }, tint = c.ink, enabled = !saving)
            Column(Modifier.weight(1f)) {
                val heading = when {
                    isAddOnly -> "Добавить треки"
                    isEdit -> "Изменить плейлист"
                    else -> "Новый плейлист"
                }
                VText(heading, size = 18.sp, weight = FontWeight.Bold, maxLines = 1)
                if (isAddOnly && !mode.addToTitle.isNullOrBlank()) {
                    VText("в «${mode.addToTitle}»", color = c.ink2, size = 13.sp, maxLines = 1)
                }
            }
        }

        if (!isAddOnly && !isEdit) {
            Row(
                Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 10.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                MoodChip("Детали", creatorTab == CreatorTab.Details) { creatorTab = CreatorTab.Details }
                val tracksLabel = if (draft.isEmpty()) "Треки" else "Треки · ${draft.size}"
                MoodChip(tracksLabel, creatorTab == CreatorTab.Tracks) { creatorTab = CreatorTab.Tracks }
            }
        }
        HLine()

        if (ed == null) {
            Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) {
                Spinner(24.dp, c.ink2)
            }
        } else if (isAddOnly) {
            TrackPickerPane(
                ed = ed,
                draftIds = draftIds,
                modifier = Modifier.weight(1f).fillMaxWidth(),
            )
        } else if (isEdit || creatorTab == CreatorTab.Details) {
            LazyColumn(
                Modifier.weight(1f).fillMaxWidth(),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                item(key = "cover") {
                    Row(
                        Modifier.fillMaxWidth().clip(RS).background(c.fill).padding(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(14.dp),
                    ) {
                        val coverModel: Any? = ed.coverPath?.let { File(it) } ?: existingPlaylist?.artworkUrl
                        Cover(null, 84.dp, radius = 8.dp, model = coverModel)
                        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                            VText("Обложка", size = 14.5.sp, weight = FontWeight.SemiBold)
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                Pill(
                                    if (coverModel != null) "Заменить" else "Выбрать",
                                    icon = VIcons.Album,
                                    height = 34.dp,
                                    enabled = !saving,
                                ) { coverPicker.launch("image/*") }
                                if (ed.coverPath != null) {
                                    Pill("Убрать", height = 34.dp, enabled = !saving) { core.editorClearCover() }
                                }
                            }
                        }
                    }
                }

                item(key = "title") {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        VText("Название", color = c.ink2, size = 13.sp, weight = FontWeight.SemiBold)
                        VField(title, { title = it }, "Название плейлиста", enabled = !saving)
                    }
                }

                item(key = "desc") {
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        VText("Описание", color = c.ink2, size = 13.sp, weight = FontWeight.SemiBold)
                        VField(description, { description = it }, "Необязательное описание", singleLine = false, enabled = !saving)
                    }
                }

                item(key = "hidden") {
                    PrefCard(
                        "Скрытый плейлист",
                        "Не отображается в поиске и рекомендациях ВКонтакте",
                    ) {
                        TogglePill(ed.hidden) { core.editorSetHidden(!ed.hidden) }
                    }
                }

                ed.error?.takeIf { it.isNotBlank() }?.let { err ->
                    item(key = "err") {
                        VText(err, color = c.danger, size = 13.5.sp)
                    }
                }

                if (!isEdit) {
                    item(key = "draft-h") {
                        Row(
                            Modifier.fillMaxWidth().padding(top = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                        ) {
                            VText(
                                if (draft.isEmpty()) "В плейлисте" else "В плейлисте · ${draft.size}",
                                Modifier.weight(1f),
                                size = 16.sp,
                                weight = FontWeight.Bold,
                            )
                            if (draft.isNotEmpty()) {
                                Pill("Очистить", height = 32.dp, enabled = !saving) { core.editorClearTracks() }
                            }
                            Pill("Выбрать треки", icon = VIcons.Plus, height = 32.dp, enabled = !saving) {
                                creatorTab = CreatorTab.Tracks
                            }
                        }
                    }
                    if (draft.isEmpty()) {
                        item(key = "draft-empty") {
                            EmptyState(
                                VIcons.Album,
                                "Треки пока не выбраны",
                                "Можно создать пустой плейлист или перейти на вкладку «Треки»",
                            )
                        }
                    } else {
                        itemsIndexed(draft, key = { i, t -> "dr-$i-${t.fullId}" }) { i, t ->
                            Row(
                                Modifier
                                    .fillMaxWidth()
                                    .clip(RS)
                                    .background(c.fill)
                                    .padding(start = 10.dp, end = 4.dp, top = 6.dp, bottom = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.spacedBy(10.dp),
                            ) {
                                Cover(t.thumbUrl, 40.dp)
                                Column(Modifier.weight(1f)) {
                                    VText(t.title, size = 14.sp, weight = FontWeight.SemiBold, maxLines = 1)
                                    VText(t.artist, color = c.ink2, size = 12.5.sp, maxLines = 1)
                                }
                                if (i > 0) {
                                    IconBtn(VIcons.ChevUp, { core.editorMoveTrack(i.toUInt(), (i - 1).toUInt()) }, size = 32.dp, iconSize = 15.dp, enabled = !saving)
                                }
                                if (i < draft.lastIndex) {
                                    IconBtn(VIcons.ChevDown, { core.editorMoveTrack(i.toUInt(), (i + 1).toUInt()) }, size = 32.dp, iconSize = 15.dp, enabled = !saving)
                                }
                                IconBtn(VIcons.Close, { core.editorToggleTrack(t.fullId) }, size = 32.dp, iconSize = 15.dp, enabled = !saving)
                            }
                        }
                    }
                }
            }
        } else {
            TrackPickerPane(
                ed = ed,
                draftIds = draftIds,
                modifier = Modifier.weight(1f).fillMaxWidth(),
            )
        }

        HLine()
        Row(
            Modifier.fillMaxWidth().background(c.raised).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            if (isAddOnly) {
                if (draft.isNotEmpty()) {
                    Pill("Сбросить", enabled = !saving) { core.editorClearTracks() }
                } else {
                    VText("Выберите треки", Modifier.weight(1f), color = c.ink2, size = 13.5.sp)
                }
                if (draft.isNotEmpty()) Spacer(Modifier.weight(1f))
                val label = when {
                    saving -> "Добавление..."
                    draft.isNotEmpty() -> "Добавить · ${draft.size}"
                    else -> "Добавить"
                }
                val canAdd = draft.isNotEmpty() && !saving
                Row(
                    Modifier
                        .height(40.dp)
                        .clip(RS)
                        .background(if (canAdd) c.ctaBg else c.fill2)
                        .tap(enabled = canAdd, pressed = c.ctaHover) { core.submitAddTracks(mode.addTo) }
                        .padding(horizontal = 18.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (saving) Spinner(16.dp, c.ctaFg)
                    VText(label, color = if (canAdd) c.ctaFg else c.ink3, size = 14.5.sp, weight = FontWeight.Bold)
                }
            } else {
                val hint = when {
                    title.isBlank() -> "Введите название"
                    !isEdit && draft.isNotEmpty() -> "Выбрано: ${formatTracksCount(draft.size.toLong())}"
                    else -> ""
                }
                VText(hint, Modifier.weight(1f), color = c.ink2, size = 13.sp, maxLines = 1)
                val canSave = title.isNotBlank() && !saving
                val label = when {
                    saving && isEdit -> "Сохранение..."
                    saving -> "Создание..."
                    isEdit -> "Сохранить"
                    draft.isNotEmpty() -> "Создать · ${draft.size}"
                    else -> "Создать"
                }
                Row(
                    Modifier
                        .height(40.dp)
                        .clip(RS)
                        .background(if (canSave) c.ctaBg else c.fill2)
                        .tap(enabled = canSave, pressed = c.ctaHover) {
                            core.saveEditor(mode.editingKey, title.trim(), description.trim())
                        }
                        .padding(horizontal = 18.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (saving) Spinner(16.dp, c.ctaFg)
                    VText(label, color = if (canSave) c.ctaFg else c.ink3, size = 14.5.sp, weight = FontWeight.Bold)
                }
            }
        }
    }
}

@Composable
private fun TrackPickerPane(ed: FfiEditor, draftIds: Set<String>, modifier: Modifier) {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val lib by app.store.library.collectAsState()
    var source by rememberSaveable { mutableStateOf(PickerSource.Library) }
    var libQuery by rememberSaveable { mutableStateOf("") }
    var vkQuery by rememberSaveable { mutableStateOf("") }
    val plListState = rememberLazyListState()

    LaunchedEffect(source) {
        if (source == PickerSource.Library && lib.tracks.isEmpty()) core.loadLibrary(false)
        if (source == PickerSource.Playlists && ed.pickerPlaylists.isEmpty() && ed.pickerPlaylist == null) {
            core.editorPickPlaylist(null)
        }
    }

    NearEndEffect(plListState) {
        if (source == PickerSource.Playlists && ed.pickerMore && ed.pickerState !is FfiLoad.Loading) {
            core.editorLoadMore()
        }
    }

    Column(modifier) {
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            MoodChip("Моя музыка", source == PickerSource.Library) { source = PickerSource.Library }
            MoodChip("Плейлисты", source == PickerSource.Playlists) { source = PickerSource.Playlists }
            MoodChip("Поиск VK", source == PickerSource.Search) { source = PickerSource.Search }
        }

        when (source) {
            PickerSource.Library -> {
                val filtered = remember(lib.tracks, libQuery) {
                    val n = libQuery.trim().lowercase()
                    if (n.isEmpty()) lib.tracks
                    else lib.tracks.filter { it.title.lowercase().contains(n) || it.artist.lowercase().contains(n) }
                }
                Row(
                    Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    VField(
                        libQuery, { libQuery = it }, "Фильтр по Моей музыке",
                        Modifier.weight(1f), leading = VIcons.Search, height = 40.dp,
                        onClear = { libQuery = "" },
                    )
                    if (filtered.isNotEmpty()) {
                        val allIn = filtered.all { it.fullId in draftIds }
                        Pill(if (allIn) "Убрать все" else "Все", height = 40.dp) {
                            val ids = filtered.map { it.fullId }
                            if (allIn) core.editorRemoveTracks(ids) else core.editorAddTracks(ids)
                        }
                    }
                }
                LazyColumn(Modifier.weight(1f).fillMaxWidth()) {
                    if (filtered.isEmpty()) {
                        item(key = "empty") {
                            EmptyState(VIcons.Search, "Нет треков", "Попробуйте другой фильтр или вкладку «Поиск VK»")
                        }
                    } else {
                        items(filtered, key = { "plb-${it.fullId}" }) { t ->
                            PickerTrackRow(t, t.fullId in draftIds) { core.editorToggleTrack(t.fullId) }
                        }
                        if (lib.loadingMore) {
                            item(key = "more-spin") {
                                Box(Modifier.fillMaxWidth().padding(14.dp), contentAlignment = Alignment.Center) {
                                    Spinner(18.dp, c.ink2)
                                }
                            }
                        } else if (lib.canLoadMore) {
                            item(key = "more") {
                                Row(Modifier.fillMaxWidth().padding(14.dp), horizontalArrangement = Arrangement.Center) {
                                    Pill("Загрузить ещё треки") { core.loadMoreLibrary() }
                                }
                            }
                        }
                    }
                }
            }

            PickerSource.Playlists -> {
                val drilled = ed.pickerPlaylist
                if (drilled != null) {
                    Row(
                        Modifier.fillMaxWidth().padding(start = 6.dp, end = 16.dp, top = 6.dp, bottom = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        IconBtn(VIcons.ChevLeft, { core.editorPickPlaylist(null) }, size = 34.dp, iconSize = 18.dp, tint = c.ink)
                        VText(drilled.title, Modifier.weight(1f), size = 15.sp, weight = FontWeight.SemiBold, maxLines = 1)
                        if (ed.pickerTracks.isNotEmpty()) {
                            val allIn = ed.pickerTracks.all { it.fullId in draftIds }
                            Pill(if (allIn) "Убрать все" else "Добавить все", height = 32.dp) {
                                val ids = ed.pickerTracks.map { it.fullId }
                                if (allIn) core.editorRemoveTracks(ids) else core.editorAddTracks(ids)
                            }
                        }
                    }
                    LazyColumn(Modifier.weight(1f).fillMaxWidth(), state = plListState) {
                        if (ed.pickerTracks.isEmpty() && ed.pickerState is FfiLoad.Loading) {
                            item(key = "skel") { TrackListSkeleton(6) }
                        } else if (ed.pickerTracks.isEmpty()) {
                            item(key = "empty") { EmptyState(VIcons.Album, "Нет треков", "В этом плейлисте нет доступных треков") }
                        } else {
                            items(ed.pickerTracks, key = { "ppt-${it.fullId}" }) { t ->
                                PickerTrackRow(t, t.fullId in draftIds) { core.editorToggleTrack(t.fullId) }
                            }
                            if (ed.pickerMore) {
                                item(key = "more") {
                                    Row(Modifier.fillMaxWidth().padding(14.dp), horizontalArrangement = Arrangement.Center) {
                                        Pill("Загрузить ещё") { core.editorLoadMore() }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    LazyColumn(Modifier.weight(1f).fillMaxWidth(), state = plListState) {
                        if (ed.pickerPlaylists.isEmpty() && ed.pickerState is FfiLoad.Loading) {
                            item(key = "skel") { TrackListSkeleton(6) }
                        } else if (ed.pickerPlaylists.isEmpty()) {
                            item(key = "empty") { EmptyState(VIcons.Album, "Нет плейлистов", "У вас пока нет сохранённых плейлистов") }
                        } else {
                            items(ed.pickerPlaylists, key = { "ppl-${it.key}" }) { p ->
                                Row(
                                    Modifier.fillMaxWidth().tap(shape = Square) { core.editorPickPlaylist(p.key) }
                                        .padding(horizontal = 16.dp, vertical = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                                ) {
                                    Cover(p.artworkUrl, 44.dp)
                                    Column(Modifier.weight(1f)) {
                                        VText(p.title, size = 15.sp, weight = FontWeight.SemiBold, maxLines = 1)
                                        VText(formatTracksCount(p.count), color = c.ink2, size = 13.sp)
                                    }
                                    VIcon(VIcons.ChevRight, 18.dp, c.ink3)
                                }
                            }
                        }
                    }
                }
            }

            PickerSource.Search -> {
                VField(
                    value = vkQuery,
                    onChange = {
                        vkQuery = it
                        core.editorSearch(it, false)
                    },
                    placeholder = "Поиск по каталогу ВКонтакте",
                    modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
                    leading = VIcons.Search,
                    height = 40.dp,
                    imeAction = ImeAction.Search,
                    onDone = { core.editorSearch(vkQuery, true) },
                    onClear = {
                        vkQuery = ""
                        core.editorClearSearch()
                    },
                )
                LazyColumn(Modifier.weight(1f).fillMaxWidth()) {
                    when {
                        vkQuery.isBlank() -> item(key = "hint") {
                            EmptyState(VIcons.Search, "Поиск по каталогу ВКонтакте", "Введите название трека или имя исполнителя")
                        }
                        ed.searchState is FfiLoad.Loading && ed.searchTracks.isEmpty() -> item(key = "skel") {
                            TrackListSkeleton(6)
                        }
                        ed.searchState is FfiLoad.Failed && ed.searchTracks.isEmpty() -> item(key = "err") {
                            ErrorState("Ошибка поиска", (ed.searchState as FfiLoad.Failed).message) { core.editorSearch(vkQuery, true) }
                        }
                        ed.searchTracks.isEmpty() -> item(key = "none") {
                            EmptyState(VIcons.Search, "Совпадений не найдено", "Попробуйте другой запрос")
                        }
                        else -> {
                            items(ed.searchTracks, key = { "psr-${it.fullId}" }) { t ->
                                PickerTrackRow(t, t.fullId in draftIds) { core.editorToggleTrack(t.fullId) }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun PickerTrackRow(track: FfiTrack, selected: Boolean, onToggle: () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .background(if (selected) c.active else androidx.compose.ui.graphics.Color.Transparent)
            .tap(shape = Square, onClick = onToggle)
            .padding(horizontal = 16.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Cover(track.thumbUrl, 42.dp)
        Column(Modifier.weight(1f)) {
            VText(track.title, size = 14.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
            VText(track.artist, color = c.ink2, size = 13.sp, maxLines = 1)
        }
        Box(
            Modifier
                .size(24.dp)
                .clip(CircleShape)
                .background(if (selected) c.ctaBg else c.fill2)
                .border(1.dp, if (selected) c.ctaBg else c.line, CircleShape),
            contentAlignment = Alignment.Center,
        ) {
            if (selected) VIcon(VIcons.Check, 14.dp, c.ctaFg) else VIcon(VIcons.Plus, 14.dp, c.ink2)
        }
    }
}

@Composable
fun UploadScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val context = LocalContext.current
    val up = snapshotOf(app.store.uploadRev) { core.upload() }
    val inFlight = up?.inFlight == true
    val items = up?.items.orEmpty()

    BackHandler(enabled = true) {
        if (!inFlight) {
            app.uploadOpen = false
            core.clearUpload()
        }
    }

    val filePicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { uris ->
        if (uris.isNotEmpty()) {
            app.store.scope.launch(Dispatchers.IO) {
                val paths = uris.mapNotNull { copyUploadUri(context, it) }
                if (paths.isNotEmpty()) core.setUploadFiles(paths)
            }
        }
    }

    Column(
        Modifier
            .fillMaxSize()
            .background(c.bg)
            .statusBarsPadding()
            .navigationBarsPadding()
            .imePadding(),
    ) {
        Row(
            Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            IconBtn(VIcons.Close, {
                if (!inFlight) {
                    app.uploadOpen = false
                    core.clearUpload()
                }
            }, tint = c.ink, enabled = !inFlight)
            VText("Загрузка аудио", Modifier.weight(1f), size = 18.sp, weight = FontWeight.Bold)
        }
        HLine()

        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item(key = "desc") {
                VText(
                    "Выберите MP3-файлы и проверьте исполнителя и название перед отправкой в Мою музыку.",
                    color = c.ink2,
                    size = 13.5.sp,
                    lineHeight = 19.sp,
                )
            }
            if (items.isEmpty()) {
                item(key = "empty") {
                    EmptyState(
                        VIcons.Upload,
                        "Файлы не выбраны",
                        "Выберите один или несколько аудиофайлов на устройстве",
                    ) {
                        PrimaryPill(VIcons.Folder, "Выбрать файлы") {
                            filePicker.launch(arrayOf("audio/mpeg", "audio/*"))
                        }
                    }
                }
            } else {
                itemsIndexed(items, key = { i, it -> "up-$i-${it.fileName}" }) { i, item ->
                    var artist by remember(item.fileName, item.artist) { mutableStateOf(item.artist) }
                    var title by remember(item.fileName, item.title) { mutableStateOf(item.title) }
                    val editable = !inFlight && (item.status is FfiUploadStatus.Pending || item.status is FfiUploadStatus.Failed)
                    val (statusText, statusColor) = when (val st = item.status) {
                        FfiUploadStatus.Pending -> "Готов к отправке" to c.ink2
                        FfiUploadStatus.Uploading -> "Загрузка..." to c.hi
                        FfiUploadStatus.Done -> "Загружено" to c.liveDot
                        is FfiUploadStatus.Failed -> "Ошибка: ${st.message}" to c.danger
                    }
                    Column(
                        Modifier.fillMaxWidth().clip(RS).background(c.fill).padding(12.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                            VText(item.fileName, Modifier.weight(1f), color = c.ink2, size = 12.5.sp, weight = FontWeight.SemiBold, maxLines = 1)
                            VText(statusText, color = statusColor, size = 12.sp, weight = FontWeight.SemiBold, maxLines = 1)
                        }
                        VField(
                            artist,
                            {
                                artist = it
                                core.setUploadTags(i.toUInt(), artist, title)
                            },
                            "Исполнитель",
                            height = 40.dp,
                            enabled = editable,
                        )
                        VField(
                            title,
                            {
                                title = it
                                core.setUploadTags(i.toUInt(), artist, title)
                            },
                            "Название",
                            height = 40.dp,
                            enabled = editable,
                        )
                    }
                }
            }
        }

        HLine()
        Row(
            Modifier.fillMaxWidth().background(c.raised).padding(horizontal = 16.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Pill("Выбрать файлы", icon = VIcons.Folder, enabled = !inFlight) {
                filePicker.launch(arrayOf("audio/mpeg", "audio/*"))
            }
            Spacer(Modifier.weight(1f))
            val canUpload = items.isNotEmpty() && !inFlight
            Row(
                Modifier
                    .height(40.dp)
                    .clip(RS)
                    .background(if (canUpload) c.ctaBg else c.fill2)
                    .tap(enabled = canUpload, pressed = c.ctaHover) { core.confirmUpload() }
                    .padding(horizontal = 18.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (inFlight) Spinner(16.dp, c.ctaFg) else VIcon(VIcons.Upload, 17.dp, if (canUpload) c.ctaFg else c.ink3)
                VText(
                    if (inFlight) "Загрузка..." else "Загрузить",
                    color = if (canUpload || inFlight) c.ctaFg else c.ink3,
                    size = 14.5.sp,
                    weight = FontWeight.Bold,
                )
            }
        }
    }
}

private fun encodeCoverJpeg(context: Context, uri: Uri): Pair<ByteArray, String>? = runCatching {
    val bmp = context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it) } ?: return null
    val maxSide = 1024
    val scaled = if (bmp.width > maxSide || bmp.height > maxSide) {
        val ratio = maxSide.toFloat() / maxOf(bmp.width, bmp.height)
        Bitmap.createScaledBitmap(bmp, (bmp.width * ratio).toInt().coerceAtLeast(1), (bmp.height * ratio).toInt().coerceAtLeast(1), true)
    } else bmp
    val baos = ByteArrayOutputStream()
    scaled.compress(Bitmap.CompressFormat.JPEG, 90, baos)
    if (scaled !== bmp) scaled.recycle()
    bmp.recycle()
    val bytes = baos.toByteArray()
    val out = File(context.cacheDir, "editor-cover-${System.currentTimeMillis()}.jpg")
    out.writeBytes(bytes)
    bytes to out.absolutePath
}.getOrNull()

private fun copyUploadUri(context: Context, uri: Uri): String? = runCatching {
    val dir = File(context.cacheDir, "uploads").apply { mkdirs() }
    var name = "audio-${System.currentTimeMillis()}.mp3"
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cur ->
        if (cur.moveToFirst()) {
            val n = cur.getString(0)
            if (!n.isNullOrBlank()) name = n.replace('/', '_')
        }
    }
    val dest = File(dir, name)
    context.contentResolver.openInputStream(uri)?.use { input ->
        dest.outputStream().use { output -> input.copyTo(output) }
    } ?: return null
    dest.absolutePath
}.getOrNull()
