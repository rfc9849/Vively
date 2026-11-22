package eu.r3pr3ss10n.vively.ui

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.remember
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.lerp
import eu.r3pr3ss10n.vively.core.heroBannerColor
import eu.r3pr3ss10n.vively.core.placeholderColors
import eu.r3pr3ss10n.vively.core.resolvePalette
import eu.r3pr3ss10n.vively.core.waveHeroColor

/** Vively design tokens, resolved in Rust with the desktop math. */
@Immutable
data class VivelyColors(
    val isDark: Boolean,
    val tintHex: String?,
    val base: Color,
    val bg: Color,
    val rail: Color,
    val raised: Color,
    val stage: Color,
    val dock: Color,
    val fill: Color,
    val fill2: Color,
    val hover: Color,
    val active: Color,
    val line: Color,
    val artRing: Color,
    val ink: Color,
    val ink2: Color,
    val ink3: Color,
    val hi: Color,
    val ctaBg: Color,
    val ctaFg: Color,
    val ctaHover: Color,
    val liveDot: Color,
    val danger: Color,
    val scrim: Color,
    val wave: Color,
    val phTop: Color,
    val phBottom: Color,
) {
    /** Hero banner background tinted by a collection accent. */
    fun hero(collectionHex: String?): Color = c(heroBannerColor(isDark, tintHex, collectionHex))

    /** Top colour of a mobile hero gradient: the accent mixed strongly into the ground. */
    fun heroTop(collectionHex: String?): Color {
        val accent = hexColor(collectionHex) ?: return hero(null)
        return lerp(bg, accent, if (isDark) 0.56f else 0.34f)
    }
}

private fun c(argb: UInt) = Color(argb.toInt())

fun vivelyColors(dark: Boolean, tintHex: String?): VivelyColors {
    val p = resolvePalette(dark, tintHex)
    val ph = placeholderColors(dark, tintHex)
    return VivelyColors(
        isDark = p.isDark, tintHex = tintHex,
        base = c(p.base), bg = c(p.ground), rail = c(p.rail), raised = c(p.raised), stage = c(p.stageBg),
        dock = c(p.dock), fill = c(p.fill), fill2 = c(p.fill2), hover = c(p.hover), active = c(p.active),
        line = c(p.line), artRing = c(p.artRing), ink = c(p.ink), ink2 = c(p.ink2), ink3 = c(p.ink3),
        hi = c(p.hi), ctaBg = c(p.ctaBg), ctaFg = c(p.ctaFg), ctaHover = c(p.ctaHover),
        liveDot = c(p.liveDot), danger = c(p.danger), scrim = c(p.scrim),
        wave = c(waveHeroColor(dark, tintHex)), phTop = c(ph[0]), phBottom = c(ph[1]),
    )
}

val LocalVivelyColors = staticCompositionLocalOf<VivelyColors> { error("VivelyTheme not set") }

object VivelyTheme {
    val colors: VivelyColors
        @Composable @ReadOnlyComposable get() = LocalVivelyColors.current
}

@Composable
fun VivelyTheme(dark: Boolean, tintHex: String?, content: @Composable () -> Unit) {
    val colors = remember(dark, tintHex) { vivelyColors(dark, tintHex) }
    CompositionLocalProvider(LocalVivelyColors provides colors, content = content)
}
