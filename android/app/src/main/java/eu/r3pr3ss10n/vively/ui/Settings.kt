package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import eu.r3pr3ss10n.vively.ThemePrefs
import eu.r3pr3ss10n.vively.core.FfiAppearance
import eu.r3pr3ss10n.vively.themePrefs
import eu.r3pr3ss10n.vively.core.cacheLimitOptions
import eu.r3pr3ss10n.vively.core.formatGb
import kotlin.math.roundToInt

@Composable
fun SettingsScreen() {
    val app = LocalApp.current
    val c = VivelyTheme.colors
    val core = app.core
    val settings by app.store.settings.collectAsState()
    val auth by app.store.auth.collectAsState()
    val dl = snapshotOf(app.store.downloadsRev) { core.downloads() }
    val cacheOptions = remember { cacheLimitOptions() }
    val ctx = LocalContext.current
    val uriHandler = LocalUriHandler.current
    val versionName = remember(ctx) {
        runCatching { ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName }.getOrNull() ?: "0.1.0"
    }

    Column(Modifier.fillMaxSize()) {
        TopBar("Настройки", titleVisible = true, onBack = { app.nav.pop() })

        LazyColumn(
            Modifier.weight(1f).fillMaxWidth(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = dockPad()),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item(key = "sec-gen") { SettingsGroupLabel("Основные") }

            item(key = "theme") {
                Column(
                    Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    VText("Тема", size = 14.5.sp, weight = FontWeight.SemiBold)
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        SegOption("Тёмная", settings.appearance == FfiAppearance.DARK, Modifier.weight(1f)) {
                            core.setAppearance(FfiAppearance.DARK)
                        }
                        SegOption("Светлая", settings.appearance == FfiAppearance.LIGHT, Modifier.weight(1f)) {
                            core.setAppearance(FfiAppearance.LIGHT)
                        }
                        SegOption("Авто", settings.appearance == FfiAppearance.SYSTEM, Modifier.weight(1f)) {
                            core.setAppearance(FfiAppearance.SYSTEM)
                        }
                    }
                    if (ThemePrefs.supported) {
                        val prefs = LocalContext.current.themePrefs
                        VText("Цвета", Modifier.padding(top = 6.dp), size = 14.5.sp, weight = FontWeight.SemiBold)
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            SegOption("Обложка", !prefs.systemColors, Modifier.weight(1f)) { prefs.updateSystemColors(false) }
                            SegOption("Система", prefs.systemColors, Modifier.weight(1f)) { prefs.updateSystemColors(true) }
                        }
                        VText(
                            if (prefs.systemColors) "Акцент из обоев и цветов Android" else "Акцент меняется под обложку трека",
                            color = c.ink2,
                            size = 13.sp,
                        )
                    }
                }
            }

            item(key = "xfade") {
                val xfade = settings.crossfadeSecs
                val xfadeLabel = if (xfade > 0.0) "Кроссфейд (${xfade.roundToInt()} с)" else "Кроссфейд (Выкл)"
                Column(
                    Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                        Column(Modifier.weight(1f)) {
                            VText(xfadeLabel, size = 14.5.sp, weight = FontWeight.SemiBold)
                            VText("Плавно смешивает конец трека с началом следующего", Modifier.padding(top = 2.dp), color = c.ink2, size = 13.sp, lineHeight = 17.sp)
                        }
                        TogglePill(xfade > 0.0) {
                            core.setCrossfade(if (xfade > 0.0) 0.0 else 4.0)
                        }
                    }
                    VSlider(
                        value = (xfade / 12.0).toFloat().coerceIn(0f, 1f),
                        onCommit = { f -> core.setCrossfade((f * 12.0).roundToInt().toDouble()) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }

            item(key = "genius") {
                PrefCard(
                    "Внешние источники текстов",
                    "Синхронизированные и обычные тексты из Musixmatch и Genius",
                ) {
                    TogglePill(settings.geniusFallback) { core.toggleGeniusFallback() }
                }
            }

            item(key = "sec-dl") { SettingsGroupLabel("Загрузки и кэш", topPad = 12) }

            item(key = "autocache") {
                PrefCard(
                    "Автозагрузка лайков",
                    "Треки из Моей музыки сохраняются при прослушивании",
                ) {
                    TogglePill(settings.autoCache) { core.toggleAutoCache() }
                }
            }

            item(key = "wifi") {
                PrefCard(
                    "Только по Wi-Fi",
                    "Не тратить мобильный трафик на автозагрузку",
                ) {
                    TogglePill(settings.wifiOnly) { core.toggleWifiOnly() }
                }
            }

            item(key = "limit") {
                Column(
                    Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Column {
                        VText("Лимит кэша", size = 14.5.sp, weight = FontWeight.SemiBold)
                        VText("Скачанное вручную не удаляется лимитом", Modifier.padding(top = 2.dp), color = c.ink2, size = 13.sp)
                    }
                    Wrap(spacing = 6.dp) {
                        cacheOptions.forEach { lim ->
                            val label = if (lim == 0L) "Без лимита" else "${lim / (1024L * 1024L * 1024L)} ГБ"
                            MoodChip(label, settings.cacheLimitBytes == lim) { core.setCacheLimit(lim) }
                        }
                    }
                }
            }

            item(key = "storage") {
                val totalBytes = dl?.totalBytes ?: 0L
                val pinnedBytes = dl?.pinnedBytes ?: 0L
                val autoBytes = (totalBytes - pinnedBytes).coerceAtLeast(0L)
                val denom = if (settings.cacheLimitBytes > 0L) {
                    maxOf(settings.cacheLimitBytes, totalBytes, 1L).toFloat()
                } else {
                    maxOf(totalBytes, 1024L * 1024L * 1024L).toFloat()
                }
                val pinPct = (pinnedBytes / denom).coerceIn(0f, 1f)
                val autoPct = (autoBytes / denom).coerceIn(0f, 1f - pinPct)

                Column(
                    Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                        VText("Хранилище", size = 14.5.sp, weight = FontWeight.SemiBold)
                        VText("${formatGb(totalBytes)}, закреплено ${formatGb(pinnedBytes)}", color = c.ink2, size = 12.5.sp)
                    }
                    Row(Modifier.fillMaxWidth().height(8.dp).clip(RS).background(c.fill2)) {
                        if (pinPct > 0f) Box(Modifier.fillMaxWidth(pinPct).fillMaxHeight().background(c.hi))
                        if (autoPct > 0f) Box(Modifier.fillMaxWidth(autoPct / (1f - pinPct).coerceAtLeast(0.001f)).fillMaxHeight().background(c.ink3))
                    }
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Pill("Очистить автокэш", Modifier.weight(1f), height = 36.dp) { core.clearAutoCache() }
                        Pill("Удалить всё", Modifier.weight(1f), height = 36.dp, danger = true) {
                            app.overlay.open { ConfirmClearDownloadsSheet() }
                        }
                    }
                }
            }

            item(key = "sec-acct") { SettingsGroupLabel("Аккаунт ВКонтакте", topPad = 12) }

            item(key = "bcast") {
                PrefCard(
                    "Транслировать в статус",
                    "Играющий трек виден в вашем статусе ВКонтакте",
                ) {
                    TogglePill(settings.broadcast) { core.toggleBroadcast() }
                }
            }

            item(key = "report") {
                PrefCard(
                    "Учитывать прослушивания",
                    "Попадают в Недавно прослушанное и влияют на рекомендации",
                ) {
                    TogglePill(settings.reporting) { core.toggleReporting() }
                }
            }

            item(key = "acct") {
                Row(
                    Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Avatar(auth.photoUrl, auth.userId, auth.userName ?: "VK", 44.dp)
                    Column(Modifier.weight(1f)) {
                        VText(auth.userName ?: "Аккаунт VK", size = 15.sp, weight = FontWeight.SemiBold, maxLines = 1)
                        VText("ВКонтакте", color = c.ink2, size = 13.sp)
                    }
                    Pill("Выйти", height = 36.dp, danger = true) {
                        app.overlay.open {
                            SheetTitle("Выйти из аккаунта?", "Текущая сессия будет завершена.")
                            Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                                Pill("Отмена", Modifier.weight(1f)) { app.overlay.close() }
                                Pill("Выйти", Modifier.weight(1f), danger = true) {
                                    app.overlay.close()
                                    core.signOut()
                                }
                            }
                        }
                    }
                }
            }

            item(key = "sec-about") { SettingsGroupLabel("О приложении", topPad = 12) }

            item(key = "about-app") {
                Row(
                    Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Box(
                        Modifier.size(40.dp).clip(RoundedCornerShape(10.dp)).background(c.ink),
                        contentAlignment = Alignment.Center,
                    ) { VIcon(VIcons.Logo, 22.dp, c.bg) }
                    Column(Modifier.weight(1f)) {
                        VText("Vively", size = 15.sp, weight = FontWeight.SemiBold)
                        VText("Версия $versionName", color = c.ink2, size = 13.sp)
                    }
                }
            }

            item(key = "about-dev") {
                PrefCard("Разработчик", "rfc9849") {
                    Pill("GitHub", height = 34.dp) {
                        runCatching { uriHandler.openUri("https://github.com/rfc9849") }
                    }
                }
            }

            item(key = "about-repo") {
                PrefCard("Исходный код", "github.com/rfc9849/Vively") {
                    Pill("Открыть", height = 34.dp) {
                        runCatching { uriHandler.openUri("https://github.com/rfc9849/Vively") }
                    }
                }
            }

            item(key = "about-license") {
                PrefCard("Лицензия", "MIT License") {
                    Pill("Открыть", height = 34.dp) {
                        runCatching { uriHandler.openUri("https://github.com/rfc9849/Vively/blob/main/LICENSE") }
                    }
                }
            }
        }
    }
}

@Composable
private fun SettingsGroupLabel(text: String, topPad: Int = 0) {
    VText(
        text,
        Modifier.padding(top = topPad.dp),
        color = VivelyTheme.colors.ink2,
        size = 13.sp,
        weight = FontWeight.SemiBold,
    )
}

@Composable
private fun SegOption(label: String, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Box(
        modifier
            .height(38.dp)
            .clip(CircleShape)
            .background(if (selected) c.ink else c.fill2)
            .tap(shape = CircleShape, pressed = if (selected) c.ink else c.hover, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        VText(label, color = if (selected) c.bg else c.ink, size = 13.5.sp, weight = FontWeight.SemiBold)
    }
}
