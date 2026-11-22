package eu.r3pr3ss10n.vively.ui

import android.graphics.Bitmap
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import coil3.SingletonImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.concurrent.ConcurrentHashMap
import kotlin.math.abs
import kotlin.math.floor
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

private sealed interface CachedTint {
    /** Sampled successfully; `hex == null` means monochrome/neutral cover. */
    data class Ready(val hex: String?) : CachedTint
    data object Failed : CachedTint
}

private object ArtworkTintCache {
    val map = ConcurrentHashMap<String, CachedTint>()
}

/**
 * Samples the artwork cover colour using the exact desktop algorithm
 * (`crates/vively/src/design/artwork.rs::sample_cover_bytes`).
 *
 * Returns a `#rrggbb` hex string, or `null` for neutral covers.
 * Falls back to [fallbackHex] only when [url] is null or loading fails.
 */
@Composable
fun rememberArtworkTint(
    url: String?,
    fallbackHex: String?,
    keepPrevious: Boolean = false,
): String? {
    val context = LocalContext.current
    val initial = when {
        url == null -> fallbackHex
        else -> when (val c = ArtworkTintCache.map[url]) {
            is CachedTint.Ready -> c.hex
            CachedTint.Failed -> fallbackHex
            null -> null
        }
    }
    var tint by remember { mutableStateOf(initial) }

    LaunchedEffect(url, fallbackHex) {
        if (url == null) {
            tint = fallbackHex
            return@LaunchedEffect
        }
        when (val cached = ArtworkTintCache.map[url]) {
            is CachedTint.Ready -> {
                tint = cached.hex
                return@LaunchedEffect
            }
            CachedTint.Failed -> {
                tint = fallbackHex
                return@LaunchedEffect
            }
            null -> {
                if (!keepPrevious) tint = null
            }
        }
        val sampled = withContext(Dispatchers.Default) {
            runCatching {
                val req = ImageRequest.Builder(context)
                    .data(url)
                    .size(64, 64)
                    .allowHardware(false)
                    .build()
                val res = SingletonImageLoader.get(context).execute(req)
                if (res is SuccessResult) {
                    val bmp = res.image.toBitmap()
                    CachedTint.Ready(sampleBitmapTint(bmp))
                } else {
                    CachedTint.Failed
                }
            }.getOrElse { CachedTint.Failed }
        }
        ArtworkTintCache.map[url] = sampled
        tint = when (sampled) {
            is CachedTint.Ready -> sampled.hex
            CachedTint.Failed -> fallbackHex
        }
    }
    return tint
}

/** Exact port of desktop `sample_cover_bytes`: 2x2 triangle filter + HSV clamp. */
private fun sampleBitmapTint(bmp: Bitmap): String? {
    val w = bmp.width
    val h = bmp.height
    if (w <= 0 || h <= 0) return null
    val pixels = IntArray(w * h)
    bmp.getPixels(pixels, 0, w, 0, 0, w, h)

    // Separable triangle weights to resample w×h -> 2×2 (matches image::FilterType::Triangle).
    val rx = max(1f, w / 2f)
    val ry = max(1f, h / 2f)
    val wx = Array(2) { qx ->
        val cx = (qx + 0.5f) * (w / 2f)
        FloatArray(w) { x -> max(0f, 1f - abs(x + 0.5f - cx) / rx) }
    }
    val wy = Array(2) { qy ->
        val cy = (qy + 0.5f) * (h / 2f)
        FloatArray(h) { y -> max(0f, 1f - abs(y + 0.5f - cy) / ry) }
    }

    var bestSat = -1f
    var bestR = 0f
    var bestG = 0f
    var bestB = 0f

    for (qy in 0 until 2) {
        val wyRow = wy[qy]
        for (qx in 0 until 2) {
            val wxRow = wx[qx]
            var sumR = 0f
            var sumG = 0f
            var sumB = 0f
            var sumW = 0f
            for (y in 0 until h) {
                val fy = wyRow[y]
                if (fy <= 0f) continue
                val rowOff = y * w
                for (x in 0 until w) {
                    val weight = fy * wxRow[x]
                    if (weight <= 0f) continue
                    val px = pixels[rowOff + x]
                    sumR += ((px ushr 16) and 0xFF) * weight
                    sumG += ((px ushr 8) and 0xFF) * weight
                    sumB += (px and 0xFF) * weight
                    sumW += weight
                }
            }
            if (sumW <= 1e-6f) continue
            val r = (sumR / sumW) / 255f
            val g = (sumG / sumW) / 255f
            val b = (sumB / sumW) / 255f

            val maxC = max(r, max(g, b))
            val minC = min(r, min(g, b))
            val d = maxC - minC
            val rawSat = if (maxC <= 1e-6f) 0f else d / maxC
            val hue = if (d <= 1e-6f) {
                0f
            } else {
                val rawH = when {
                    abs(maxC - r) <= 1e-6f -> (g - b) / d + if (g < b) 6f else 0f
                    abs(maxC - g) <= 1e-6f -> (b - r) / d + 2f
                    else -> (r - g) / d + 4f
                }
                rawH / 6f
            }

            val s = if (rawSat > 0.08f) rawSat.coerceIn(0.35f, 0.85f) else rawSat
            val v = maxC.coerceIn(0.22f, 0.68f)

            if (rawSat > bestSat) {
                bestSat = rawSat
                val (or, og, ob) = hsvToRgb(hue, s, v)
                bestR = or
                bestG = og
                bestB = ob
            }
        }
    }

    if (bestSat <= 0.08f) return null
    val ri = (bestR.coerceIn(0f, 1f) * 255f).roundToInt()
    val gi = (bestG.coerceIn(0f, 1f) * 255f).roundToInt()
    val bi = (bestB.coerceIn(0f, 1f) * 255f).roundToInt()
    return String.format("#%02x%02x%02x", ri, gi, bi)
}

private fun hsvToRgb(h: Float, s: Float, v: Float): Triple<Float, Float, Float> {
    if (s <= 1e-6f) return Triple(v, v, v)
    val wrapped = ((h % 1f) + 1f) % 1f
    val h6 = wrapped * 6f
    val i = floor(h6).toInt()
    val f = h6 - i
    val p = v * (1f - s)
    val q = v * (1f - s * f)
    val t = v * (1f - s * (1f - f))
    return when (i % 6) {
        0 -> Triple(v, t, p)
        1 -> Triple(q, v, p)
        2 -> Triple(p, v, t)
        3 -> Triple(p, q, v)
        4 -> Triple(t, p, v)
        else -> Triple(v, p, q)
    }
}
