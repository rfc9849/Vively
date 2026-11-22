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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.core.formatFriendsCount
import eu.r3pr3ss10n.vively.core.formatGb
import eu.r3pr3ss10n.vively.core.formatTracksCount

@Composable
fun ProfileScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    LaunchedEffect(Unit) { core.loadFriends(false) }

    val auth by app.store.auth.collectAsState()
    val lib by app.store.library.collectAsState()
    val dl = snapshotOf(app.store.downloadsRev) { core.downloads() }
    val friends = snapshotOf(app.store.friendsRev) { core.friends() }

    val dlSub = if (dl != null && dl.ready.isNotEmpty()) {
        "${formatTracksCount(dl.ready.size.toLong())} · ${formatGb(dl.totalBytes)}"
    } else {
        "Музыка без интернета"
    }
    val frCount = maxOf(friends?.total ?: 0L, friends?.items?.size?.toLong() ?: 0L)
    val frSub = if (frCount > 0) formatFriendsCount(frCount) else "Музыка и плейлисты друзей"

    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = dockPad()),
    ) {
        item(key = "head") {
            Column(
                Modifier
                    .fillMaxWidth()
                    .statusBarsPadding()
                    .padding(start = 16.dp, end = 6.dp, top = 20.dp, bottom = 20.dp),
            ) {
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(14.dp),
                ) {
                    Avatar(auth.photoUrl, auth.userId, auth.userName ?: "VK", 72.dp)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        VText(auth.userName ?: "Аккаунт VK", size = 21.sp, weight = FontWeight.Bold, maxLines = 2, lineHeight = 25.sp, tracking = (-0.2).sp)
                        val stats = buildList {
                            add("ВКонтакте")
                            if (lib.total > 0) add(formatTracksCount(lib.total))
                        }
                        VText(stats.joinToString(" · "), color = c.ink2, size = 13.5.sp, maxLines = 1)
                    }
                    IconBtn(VIcons.Settings, { app.openSettings() }, size = 44.dp, iconSize = 22.dp, tint = c.ink)
                }
            }
        }

        item(key = "nav-group") {
            Column(
                Modifier
                    .padding(horizontal = 16.dp)
                    .fillMaxWidth()
                    .clip(RL)
                    .background(c.fill),
            ) {
                ProfileRow(VIcons.Download, "Загруженное", dlSub) { app.openDownloads() }
                HLine(Modifier.padding(start = 60.dp))
                ProfileRow(VIcons.Users, "Друзья", frSub) { app.openFriends() }
            }
        }

        item(key = "actions-group") {
            Spacer(Modifier.height(12.dp))
            Column(
                Modifier
                    .padding(horizontal = 16.dp)
                    .fillMaxWidth()
                    .clip(RL)
                    .background(c.fill),
            ) {
                ProfileRow(VIcons.Plus, "Создать плейлист", "Собрать подборку треков") {
                    app.openEditor(EditorMode(null))
                }
                HLine(Modifier.padding(start = 60.dp))
                ProfileRow(VIcons.Upload, "Загрузить аудио", "Отправить MP3 в Мою музыку") {
                    app.uploadOpen = true
                }
                HLine(Modifier.padding(start = 60.dp))
                ProfileRow(VIcons.Download, "Скачать медиатеку", "Закрепить треки офлайн") {
                    app.overlay.open { BulkDownloadSheet(lib.total) }
                }
            }
        }

        item(key = "settings-group") {
            Spacer(Modifier.height(12.dp))
            Column(
                Modifier
                    .padding(horizontal = 16.dp)
                    .fillMaxWidth()
                    .clip(RL)
                    .background(c.fill),
            ) {
                ProfileRow(VIcons.Settings, "Все настройки", "Тема, кроссфейд, кэш, тексты и статус ВК") {
                    app.openSettings()
                }
            }
        }
    }
}

@Composable
private fun ProfileRow(icon: ImageVector, title: String, subtitle: String, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .tap(shape = Square, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 13.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            Modifier.size(36.dp).clip(CircleShape).background(c.fill2),
            contentAlignment = Alignment.Center,
        ) { VIcon(icon, 18.dp, c.ink) }
        Column(Modifier.weight(1f)) {
            VText(title, size = 15.sp, weight = FontWeight.SemiBold, maxLines = 1)
            VText(subtitle, color = c.ink2, size = 13.sp, maxLines = 1)
        }
        VIcon(VIcons.ChevRight, 17.dp, c.ink3)
    }
}
