package eu.r3pr3ss10n.vively.ui

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.unit.dp

private fun icon(build: ImageVector.Builder.() -> Unit): ImageVector =
    ImageVector.Builder(
        defaultWidth = 24.dp,
        defaultHeight = 24.dp,
        viewportWidth = 24f,
        viewportHeight = 24f,
    ).apply(build).build()

private fun ImageVector.Builder.p(
    d: String,
    fill: Boolean,
    sw: Float = 0f,
    cap: StrokeCap = StrokeCap.Butt,
    join: StrokeJoin = StrokeJoin.Miter,
    alpha: Float = 1f,
) {
    addPath(
        pathData = addPathNodes(d),
        fill = if (fill) SolidColor(Color.Black) else null,
        fillAlpha = alpha,
        stroke = if (sw > 0f) SolidColor(Color.Black) else null,
        strokeAlpha = alpha,
        strokeLineWidth = sw,
        strokeLineCap = cap,
        strokeLineJoin = join,
    )
}

/** The desktop icon set (crates/vively/src/design/icons.rs), drawn black and tinted at use. */
object VIcons {
    val Logo: ImageVector by lazy { icon { p("M12 3H18V7H14V17A4 4 0 1 1 12 13.536Z", true) } }
    val Home: ImageVector by lazy { icon { p("M4 10.5L12 4l8 6.5V20a1 1 0 0 1-1 1h-4.5v-5.5h-5V21H5a1 1 0 0 1-1-1v-9.5z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Round) } }
    val HomeFill: ImageVector by lazy { icon { p("M4 10.5L12 4l8 6.5V20a1 1 0 0 1-1 1h-4.5v-5.5h-5V21H5a1 1 0 0 1-1-1v-9.5z", true, 1.8f, StrokeCap.Butt, StrokeJoin.Round) } }
    val SearchBold: ImageVector by lazy { icon { p("M4.5 10.8a6.3 6.3 0 1 0 12.6 0a6.3 6.3 0 1 0 -12.6 0Z", false, 2.5f, StrokeCap.Butt, StrokeJoin.Miter); p("M15.5 15.5L20 20", false, 2.5f, StrokeCap.Round, StrokeJoin.Miter) } }
    val UserFill: ImageVector by lazy { icon { p("M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2z", true, 1.8f, StrokeCap.Round, StrokeJoin.Round); p("M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z", true, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val Search: ImageVector by lazy { icon { p("M4.5 10.8a6.3 6.3 0 1 0 12.6 0a6.3 6.3 0 1 0 -12.6 0Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M15.5 15.5L20 20", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Heart: ImageVector by lazy { icon { p("M12 20.2l-7.2-7a4.8 4.8 0 0 1 6.8-6.8l.4.4.4-.4a4.8 4.8 0 0 1 6.8 6.8l-7.2 7z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Round) } }
    val HeartFill: ImageVector by lazy { icon { p("M12 20.2l-7.2-7a4.8 4.8 0 0 1 6.8-6.8l.4.4.4-.4a4.8 4.8 0 0 1 6.8 6.8l-7.2 7z", true) } }
    val HeartSlash: ImageVector by lazy { icon { p("M12 20.2l-7.2-7a4.8 4.8 0 0 1 6.8-6.8l.4.4.4-.4a4.8 4.8 0 0 1 6.8 6.8l-7.2 7z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Round); p("M4.5 4.5l15 15", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Infinity: ImageVector by lazy { icon { p("M18.2 8.5a4 4 0 0 1 0 7c-2.2 0-4-2-6.2-3.5C9.8 10.5 8 8.5 5.8 8.5a4 4 0 0 0 0 7c2.2 0 4-2 6.2-3.5 2.2-1.5 4-3.5 6.2-3.5z", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Users: ImageVector by lazy { icon { p("M5.8 8.5a3.2 3.2 0 1 0 6.4 0a3.2 3.2 0 1 0 -6.4 0Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M3.5 19c0-3 2.5-5.2 5.5-5.2s5.5 2.2 5.5 5.2", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter); p("M14 9.2a2.5 2.5 0 1 0 5 0a2.5 2.5 0 1 0 -5 0Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M16 14.2c2.4.3 4.5 2 4.5 4.8", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Mic: ImageVector by lazy { icon { p("M12 3.5H12A3 3 0 0 1 15 6.5V11A3 3 0 0 1 12 14H12A3 3 0 0 1 9 11V6.5A3 3 0 0 1 12 3.5Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M6 11a6 6 0 0 0 12 0M12 17v3.5M9 20.5h6", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Album: ImageVector by lazy { icon { p("M7 4H17A3 3 0 0 1 20 7V17A3 3 0 0 1 17 20H7A3 3 0 0 1 4 17V7A3 3 0 0 1 7 4Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M8 12a4 4 0 1 0 8 0a4 4 0 1 0 -8 0Z", false, 1.6f, StrokeCap.Butt, StrokeJoin.Miter); p("M10.8 12a1.2 1.2 0 1 0 2.4 0a1.2 1.2 0 1 0 -2.4 0Z", true) } }
    val Trash: ImageVector by lazy { icon { p("M4.5 7h15M9.5 7V5a1 1 0 0 1 1-1h3a1 1 0 0 1 1 1v2M6.8 7l.9 12a1.5 1.5 0 0 0 1.5 1.4h5.6a1.5 1.5 0 0 0 1.5-1.4l.9-12", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val Folder: ImageVector by lazy { icon { p("M3.5 7.5a2 2 0 0 1 2-2h4l2 2.2h7a2 2 0 0 1 2 2v7.8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2V7.5z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Round) } }
    val Download: ImageVector by lazy { icon { p("M12 4v11m0 0l-4-4m4 4l4-4M5 19.5h14", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val Downloaded: ImageVector by lazy { icon { p("M3 12a9 9 0 1 0 18 0a9 9 0 1 0 -18 0Z", true, 0f, StrokeCap.Butt, StrokeJoin.Miter, 0.2f); p("M12 7.5v7m0 0l-3-3m3 3l3-3", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val Upload: ImageVector by lazy { icon { p("M12 16V5m0 0L8 9m4-4l4 4M5 19.5h14", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val Play: ImageVector by lazy { icon { p("M7.5 5.2l11.2 6.8-11.2 6.8V5.2z", true) } }
    val Pause: ImageVector by lazy { icon { p("M7.7 5H9.3A1.2 1.2 0 0 1 10.5 6.2V17.8A1.2 1.2 0 0 1 9.3 19H7.7A1.2 1.2 0 0 1 6.5 17.8V6.2A1.2 1.2 0 0 1 7.7 5Z", true); p("M14.7 5H16.3A1.2 1.2 0 0 1 17.5 6.2V17.8A1.2 1.2 0 0 1 16.3 19H14.7A1.2 1.2 0 0 1 13.5 17.8V6.2A1.2 1.2 0 0 1 14.7 5Z", true) } }
    val Prev: ImageVector by lazy { icon { p("M6 6v12M18 6l-9.5 6L18 18V6z", true, 1.6f, StrokeCap.Round, StrokeJoin.Round) } }
    val Next: ImageVector by lazy { icon { p("M18 6v12M6 6l9.5 6L6 18V6z", true, 1.6f, StrokeCap.Round, StrokeJoin.Round) } }
    val Shuffle: ImageVector by lazy { icon { p("M4 7h3.8l8.4 10H20m0 0l-2.6-2.6M20 17l-2.6 2.6M4 17h3.8l2.6-3.1M14.2 9.4L16.2 7H20m0 0l-2.6-2.6M20 7l-2.6 2.6", false, 1.7f, StrokeCap.Round, StrokeJoin.Round) } }
    val Repeat: ImageVector by lazy { icon { p("M5 10V7.5A1.5 1.5 0 0 1 6.5 6H19m0 0l-2.8-2.8M19 6l-2.8 2.8M19 14v2.5a1.5 1.5 0 0 1-1.5 1.5H5m0 0l2.8 2.8M5 18l2.8-2.8", false, 1.7f, StrokeCap.Round, StrokeJoin.Round) } }
    val Wave: ImageVector by lazy { icon { p("M4 12v2m4-6v8m4-11v14m4-9v6m4-4v2", false, 2.0f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Lyrics: ImageVector by lazy { icon { p("M5 7h14M5 12h10M5 17h12", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Queue: ImageVector by lazy { icon { p("M4 7h16M4 12h16M4 17h10", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter); p("M17 15l4 2.5-4 2.5v-5z", true) } }
    val Plus: ImageVector by lazy { icon { p("M12 5v14M5 12h14", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Check: ImageVector by lazy { icon { p("M5 12.5l4.5 4.5L19 7.5", false, 2.0f, StrokeCap.Round, StrokeJoin.Round) } }
    val Close: ImageVector by lazy { icon { p("M6 6l12 12M18 6L6 18", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val More: ImageVector by lazy { icon { p("M4.4 12a1.6 1.6 0 1 0 3.2 0a1.6 1.6 0 1 0 -3.2 0Z", true); p("M10.4 12a1.6 1.6 0 1 0 3.2 0a1.6 1.6 0 1 0 -3.2 0Z", true); p("M16.4 12a1.6 1.6 0 1 0 3.2 0a1.6 1.6 0 1 0 -3.2 0Z", true) } }
    val Sliders: ImageVector by lazy { icon { p("M4 7h2.8M11.2 7H20M4 12h8.8M17.2 12H20M4 17h3.8M12.2 17H20", false, 1.7f, StrokeCap.Round, StrokeJoin.Miter); p("M6.8 7a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0 -4.4 0Z", false, 1.7f, StrokeCap.Butt, StrokeJoin.Miter); p("M12.8 12a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0 -4.4 0Z", false, 1.7f, StrokeCap.Butt, StrokeJoin.Miter); p("M7.8 17a2.2 2.2 0 1 0 4.4 0a2.2 2.2 0 1 0 -4.4 0Z", false, 1.7f, StrokeCap.Butt, StrokeJoin.Miter) } }
    val Volume: ImageVector by lazy { icon { p("M4.5 9.5h3.2L12 5.8v12.4l-4.3-3.7H4.5v-5z", true); p("M15.5 8.8a4.5 4.5 0 0 1 0 6.4M18.2 6.2a8 8 0 0 1 0 11.6", false, 1.7f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Mute: ImageVector by lazy { icon { p("M4.5 9.5h3.2L12 5.8v12.4l-4.3-3.7H4.5v-5z", true); p("M15.5 9.5l5 5m0-5l-5 5", false, 1.7f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Expand: ImageVector by lazy { icon { p("M14 4h6v6M20 4l-6.5 6.5M10 20H4v-6M4 20l6.5-6.5", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val ChevLeft: ImageVector by lazy { icon { p("M14.5 6l-6 6 6 6", false, 1.9f, StrokeCap.Round, StrokeJoin.Round) } }
    val ChevRight: ImageVector by lazy { icon { p("M9.5 6l6 6-6 6", false, 1.9f, StrokeCap.Round, StrokeJoin.Round) } }
    val ChevUp: ImageVector by lazy { icon { p("M6 14.5l6-6 6 6", false, 1.9f, StrokeCap.Round, StrokeJoin.Round) } }
    val ChevDown: ImageVector by lazy { icon { p("M6 9.5l6 6 6-6", false, 1.9f, StrokeCap.Round, StrokeJoin.Round) } }
    val Moon: ImageVector by lazy { icon { p("M20.2 14.8A8.2 8.2 0 0 1 9.2 3.8a8.2 8.2 0 1 0 11 11z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Round) } }
    val Sun: ImageVector by lazy { icon { p("M8 12a4 4 0 1 0 8 0a4 4 0 1 0 -8 0Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M12 2.8v2.2M12 19v2.2M2.8 12H5M19 12h2.2M5.5 5.5l1.6 1.6M16.9 16.9l1.6 1.6M18.5 5.5l-1.6 1.6M7.1 16.9l-1.6 1.6", false, 1.8f, StrokeCap.Round, StrokeJoin.Miter) } }
    val Settings: ImageVector by lazy { icon { p("M10.3 3.5h3.4l.5 2.4 1.6.9 2.3-.8 1.7 2.9-1.8 1.6v1.9l1.8 1.6-1.7 2.9-2.3-.8-1.6.9-.5 2.4h-3.4l-.5-2.4-1.6-.9-2.3.8-1.7-2.9 1.8-1.6v-1.9L4.2 8.9l1.7-2.9 2.3.8 1.6-.9.5-2.4z", false, 1.7f, StrokeCap.Butt, StrokeJoin.Round); p("M9.4 12a2.6 2.6 0 1 0 5.2 0a2.6 2.6 0 1 0 -5.2 0Z", false, 1.7f, StrokeCap.Butt, StrokeJoin.Miter) } }
    val Share: ImageVector by lazy { icon { p("M12 15V4m0 0L8 8m4-4l4 4M5 14v4.5A1.5 1.5 0 0 0 6.5 20h11a1.5 1.5 0 0 0 1.5-1.5V14", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
    val Edit: ImageVector by lazy { icon { p("M15.8 4.5l3.7 3.7L8.7 19H5v-3.7L15.8 4.5z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Round) } }
    val Lock: ImageVector by lazy { icon { p("M7.5 10.5H16.5A2 2 0 0 1 18.5 12.5V17.5A2 2 0 0 1 16.5 19.5H7.5A2 2 0 0 1 5.5 17.5V12.5A2 2 0 0 1 7.5 10.5Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter) } }
    val Loader: ImageVector by lazy { icon { p("M3.5 12a8.5 8.5 0 1 0 17 0a8.5 8.5 0 1 0 -17 0Z", false, 2.2f, StrokeCap.Butt, StrokeJoin.Miter, 0.22f); p("M12 3.5A8.5 8.5 0 0 1 19.7 15.59", false, 2.2f, StrokeCap.Round, StrokeJoin.Miter) } }
    val QrCode: ImageVector by lazy { icon { p("M4.7 3.5H8.8A1.2 1.2 0 0 1 10 4.7V8.8A1.2 1.2 0 0 1 8.8 10H4.7A1.2 1.2 0 0 1 3.5 8.8V4.7A1.2 1.2 0 0 1 4.7 3.5Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M5.8 5.8h1.9v1.9h-1.9Z", true); p("M15.2 3.5H19.3A1.2 1.2 0 0 1 20.5 4.7V8.8A1.2 1.2 0 0 1 19.3 10H15.2A1.2 1.2 0 0 1 14 8.8V4.7A1.2 1.2 0 0 1 15.2 3.5Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M16.3 5.8h1.9v1.9h-1.9Z", true); p("M4.7 14H8.8A1.2 1.2 0 0 1 10 15.2V19.3A1.2 1.2 0 0 1 8.8 20.5H4.7A1.2 1.2 0 0 1 3.5 19.3V15.2A1.2 1.2 0 0 1 4.7 14Z", false, 1.8f, StrokeCap.Butt, StrokeJoin.Miter); p("M5.8 16.3h1.9v1.9h-1.9Z", true); p("M14 14h2.5v2.5H14zM18 14h2.5v2.5H18zM16 16.5h2.5V19H16zM14 18.5h2V20.5h-2zM18.5 18.5h2V20.5h-2z", true) } }
    val User: ImageVector by lazy { icon { p("M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2", false, 1.8f, StrokeCap.Round, StrokeJoin.Round); p("M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z", false, 1.8f, StrokeCap.Round, StrokeJoin.Round) } }
}
