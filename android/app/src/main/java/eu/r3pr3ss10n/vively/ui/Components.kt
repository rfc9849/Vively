@file:OptIn(ExperimentalFoundationApi::class, ExperimentalLayoutApi::class)

package eu.r3pr3ss10n.vively.ui

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import kotlin.math.roundToInt

val RS = RoundedCornerShape(8.dp)
val RL = RoundedCornerShape(14.dp)

@Composable
fun VText(
    text: String,
    modifier: Modifier = Modifier,
    color: Color = VivelyTheme.colors.ink,
    size: TextUnit = 15.sp,
    weight: FontWeight = FontWeight.Normal,
    maxLines: Int = Int.MAX_VALUE,
    align: TextAlign = TextAlign.Unspecified,
    lineHeight: TextUnit = TextUnit.Unspecified,
    tracking: TextUnit = TextUnit.Unspecified,
) {
    BasicText(
        text = text,
        modifier = modifier,
        style = TextStyle(color = color, fontSize = size, fontWeight = weight, textAlign = align, lineHeight = lineHeight, letterSpacing = tracking),
        maxLines = maxLines,
        overflow = TextOverflow.Ellipsis,
    )
}

@Composable
fun VIcon(icon: ImageVector, size: Dp = 20.dp, tint: Color = VivelyTheme.colors.ink, modifier: Modifier = Modifier) {
    Image(rememberVectorPainter(icon), null, modifier.size(size), colorFilter = ColorFilter.tint(tint))
}

@Composable
fun Spinner(size: Dp = 20.dp, tint: Color = VivelyTheme.colors.ink) {
    val t = rememberInfiniteTransition(label = "spinner")
    val angle by t.animateFloat(0f, 360f, infiniteRepeatable(tween(850, easing = LinearEasing)), label = "angle")
    VIcon(VIcons.Loader, size, tint, Modifier.graphicsLayer { rotationZ = angle })
}

/** Click and long-press; shows the hover tint while pressed. */
fun Modifier.tap(
    enabled: Boolean = true,
    shape: Shape = RS,
    pressed: Color? = null,
    onLongClick: (() -> Unit)? = null,
    onClick: () -> Unit,
): Modifier = composed {
    val src = remember { MutableInteractionSource() }
    val isPressed by src.collectIsPressedAsState()
    val c = VivelyTheme.colors
    this
        .clip(shape)
        .background(if (isPressed) pressed ?: c.hover else Color.Transparent)
        .combinedClickable(
            interactionSource = src,
            indication = null,
            enabled = enabled,
            onLongClick = onLongClick,
            onClick = onClick,
        )
}

@Composable
fun IconBtn(
    icon: ImageVector,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    size: Dp = 40.dp,
    iconSize: Dp = 20.dp,
    tint: Color = VivelyTheme.colors.ink2,
    active: Boolean = false,
    enabled: Boolean = true,
) {
    val c = VivelyTheme.colors
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(if (active) c.fill2 else Color.Transparent)
            .tap(enabled, shape = CircleShape, pressed = c.fill, onClick = onClick)
            .alpha(if (enabled) 1f else 0.4f),
        contentAlignment = Alignment.Center,
    ) {
        VIcon(icon, iconSize, if (active) c.ink else tint)
    }
}

@Composable
fun PrimaryPill(icon: ImageVector?, label: String, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        modifier
            .height(44.dp)
            .clip(CircleShape)
            .background(c.ctaBg)
            .tap(shape = CircleShape, pressed = c.ctaHover, onClick = onClick)
            .padding(start = if (icon != null) 16.dp else 20.dp, end = 20.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
    ) {
        if (icon != null) VIcon(icon, 18.dp, c.ctaFg)
        VText(label, color = c.ctaFg, size = 15.sp, weight = FontWeight.Bold, maxLines = 1)
    }
}

/** Round play button of collection headers and the wave card. */
@Composable
fun PlayFab(onClick: () -> Unit, modifier: Modifier = Modifier, size: Dp = 48.dp, playing: Boolean = false) {
    val c = VivelyTheme.colors
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(c.ctaBg)
            .tap(shape = CircleShape, pressed = c.ctaHover, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        VIcon(if (playing) VIcons.Pause else VIcons.Play, size * 0.42f, c.ctaFg)
    }
}

/** Collection action row: icon actions on the left, shuffle and play on the right. */
@Composable
fun CollectionActions(
    onPlay: (() -> Unit)?,
    onShuffle: (() -> Unit)?,
    modifier: Modifier = Modifier,
    info: List<String> = emptyList(),
    trailing: @Composable RowScope.() -> Unit = {},
    leading: @Composable RowScope.() -> Unit = {},
) {
    val c = VivelyTheme.colors
    Row(
        modifier.fillMaxWidth().padding(start = 6.dp, end = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        leading()
        if (info.isEmpty()) {
            Spacer(Modifier.weight(1f))
        } else {
            Column(Modifier.weight(1f).padding(start = 10.dp, end = 8.dp)) {
                info.forEach { VText(it, color = c.ink2, size = 14.sp, maxLines = 1) }
            }
        }
        trailing()
        if (onShuffle != null) IconBtn(VIcons.Shuffle, onShuffle, size = 48.dp, iconSize = 24.dp, tint = c.ink)
        if (onPlay != null) {
            Spacer(Modifier.width(6.dp))
            PlayFab(onPlay)
        }
    }
}

/** Icon in the left part of [CollectionActions]. */
@Composable
fun ActionIcon(icon: ImageVector, onClick: () -> Unit, tint: Color = VivelyTheme.colors.ink2, enabled: Boolean = true) {
    IconBtn(icon, onClick, size = 44.dp, iconSize = 23.dp, tint = tint, enabled = enabled)
}

@Composable
fun Pill(
    label: String,
    modifier: Modifier = Modifier,
    icon: ImageVector? = null,
    selected: Boolean = false,
    height: Dp = 40.dp,
    danger: Boolean = false,
    enabled: Boolean = true,
    onClick: () -> Unit,
) {
    val c = VivelyTheme.colors
    val fg = when {
        selected -> c.bg
        danger -> c.danger
        else -> c.ink
    }
    Row(
        modifier
            .height(height)
            .clip(CircleShape)
            .background(if (selected) c.ink else c.fill)
            .tap(enabled, shape = CircleShape, pressed = if (selected) c.ink else c.fill2, onClick = onClick)
            .alpha(if (enabled) 1f else 0.5f)
            .padding(horizontal = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp, Alignment.CenterHorizontally),
    ) {
        if (icon != null) VIcon(icon, 17.dp, fg)
        VText(label, color = fg, size = 14.sp, weight = FontWeight.SemiBold, maxLines = 1)
    }
}

@Composable
fun MoodChip(label: String, selected: Boolean, onClick: () -> Unit) {
    Pill(label, selected = selected, height = 36.dp, onClick = onClick)
}

@Composable
fun TogglePill(on: Boolean, onToggle: () -> Unit) {
    val c = VivelyTheme.colors
    Box(
        Modifier
            .size(40.dp, 24.dp)
            .clip(CircleShape)
            .background(if (on) c.hi else c.fill2)
            .tap(shape = CircleShape, onClick = onToggle),
    ) {
        Box(
            Modifier
                .padding(start = if (on) 19.dp else 3.dp, top = 3.dp)
                .size(18.dp)
                .clip(CircleShape)
                .background(if (on && c.isDark) c.ctaFg else Color.White),
        )
    }
}

@Composable
fun ExplicitBadge() {
    val c = VivelyTheme.colors
    Box(Modifier.clip(RoundedCornerShape(2.dp)).background(c.fill2).padding(horizontal = 4.dp, vertical = 1.dp)) {
        VText("E", color = c.ink2, size = 10.sp, weight = FontWeight.Bold)
    }
}

/** Artwork with the desktop placeholder (tinted gradient + logo). */
@Composable
fun Cover(
    url: String?,
    size: Dp,
    modifier: Modifier = Modifier,
    round: Boolean = false,
    radius: Dp = 6.dp,
    model: Any? = url,
) {
    val c = VivelyTheme.colors
    val shape = if (round) CircleShape else RoundedCornerShape(radius)
    Box(
        modifier
            .size(size)
            .clip(shape)
            .background(Brush.linearGradient(listOf(c.phTop, c.phBottom))),
        contentAlignment = Alignment.Center,
    ) {
        val emblem = minOf(size * 0.48f, size * 0.36f + 10.dp)
        VIcon(if (round) VIcons.Mic else VIcons.Logo, emblem, c.ink.copy(alpha = if (size < 64.dp) 0.45f else 0.42f))
        if (model != null) {
            AsyncImage(model = model, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
        }
    }
}

@Composable
fun LikedCover(size: Dp, modifier: Modifier = Modifier, radius: Dp = 6.dp) {
    val shape = RoundedCornerShape(radius)
    Box(
        modifier.size(size).clip(shape).background(Color(0xFF2A2B2E)),
        contentAlignment = Alignment.Center,
    ) {
        VIcon(VIcons.HeartFill, size * 0.48f, Color(0xFFECEAE6))
    }
}

private val AVATAR_BGS = longArrayOf(0xFF2A2A2A, 0xFF1F5F4A, 0xFF6E2E35, 0xFF354F52, 0xFF524638, 0xFF3B3B3B)

fun initials(name: String): String {
    val s = name.trim().split(' ').filter { it.isNotEmpty() }.take(2)
        .mapNotNull { it.firstOrNull()?.uppercaseChar() }.joinToString("")
    return s.ifEmpty { "VK" }
}

@Composable
fun Avatar(url: String?, seed: Long, name: String, size: Dp, modifier: Modifier = Modifier) {
    val c = VivelyTheme.colors
    Box(
        modifier
            .size(size)
            .clip(CircleShape)
            .background(Color(AVATAR_BGS[Math.floorMod(seed, AVATAR_BGS.size.toLong()).toInt()]))
            .border(1.dp, c.artRing, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        VText(initials(name), color = Color(0xFFF5F5F5), size = (size.value * 0.36f).coerceAtLeast(11f).sp, weight = FontWeight.Bold)
        if (url != null) AsyncImage(model = url, contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize())
    }
}

@Composable
fun VField(
    value: String,
    onChange: (String) -> Unit,
    placeholder: String,
    modifier: Modifier = Modifier,
    keyboardType: KeyboardType = KeyboardType.Text,
    masked: Boolean = false,
    singleLine: Boolean = true,
    imeAction: ImeAction = ImeAction.Done,
    onDone: () -> Unit = {},
    focusRequester: FocusRequester? = null,
    leading: ImageVector? = null,
    height: Dp = 46.dp,
    onClear: (() -> Unit)? = null,
    enabled: Boolean = true,
) {
    val c = VivelyTheme.colors
    val base = modifier
        .fillMaxWidth()
        .then(if (singleLine) Modifier.height(height) else Modifier.heightIn(min = 96.dp))
        .clip(RoundedCornerShape(if (singleLine) height / 2 else 14.dp))
        .background(c.fill)
    BasicTextField(
        value = value,
        onValueChange = onChange,
        modifier = if (focusRequester != null) base.focusRequester(focusRequester) else base,
        enabled = enabled,
        singleLine = singleLine,
        textStyle = TextStyle(color = c.ink, fontSize = 15.sp),
        cursorBrush = SolidColor(c.ink),
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType, imeAction = imeAction),
        keyboardActions = KeyboardActions(onAny = { onDone() }),
        visualTransformation = if (masked) PasswordVisualTransformation() else VisualTransformation.None,
        decorationBox = { inner ->
            Row(
                Modifier
                    .fillMaxWidth()
                    .then(if (singleLine) Modifier.fillMaxHeight() else Modifier)
                    .padding(
                        start = 14.dp,
                        end = if (onClear != null) 4.dp else 14.dp,
                        top = if (singleLine) 0.dp else 12.dp,
                        bottom = if (singleLine) 0.dp else 12.dp,
                    ),
                verticalAlignment = if (singleLine) Alignment.CenterVertically else Alignment.Top,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (leading != null) VIcon(leading, 17.dp, c.ink2)
                Box(Modifier.weight(1f)) {
                    if (value.isEmpty()) VText(placeholder, color = c.ink3, maxLines = if (singleLine) 1 else 3)
                    inner()
                }
                if (onClear != null && value.isNotEmpty()) IconBtn(VIcons.Close, onClear, size = 32.dp, iconSize = 15.dp)
            }
        },
    )
}

/** Seek/volume slider; reports the fraction on tap or drag release. */
@Composable
fun VSlider(
    value: Float,
    onCommit: (Float) -> Unit,
    modifier: Modifier = Modifier,
    onDrag: (Float?) -> Unit = {},
) {
    val c = VivelyTheme.colors
    val commit by rememberUpdatedState(onCommit)
    val dragCb by rememberUpdatedState(onDrag)
    var width by remember { mutableIntStateOf(1) }
    var drag by remember { mutableStateOf<Float?>(null) }
    val f = (drag ?: value).coerceIn(0f, 1f)
    Box(
        modifier
            .height(28.dp)
            .onSizeChanged { width = it.width.coerceAtLeast(1) }
            .pointerInput(Unit) { detectTapGestures { o -> commit((o.x / width).coerceIn(0f, 1f)) } }
            .pointerInput(Unit) {
                detectHorizontalDragGestures(
                    onDragStart = { o ->
                        drag = (o.x / width).coerceIn(0f, 1f)
                        dragCb(drag)
                    },
                    onDragEnd = {
                        drag?.let(commit)
                        drag = null
                        dragCb(null)
                    },
                    onDragCancel = {
                        drag = null
                        dragCb(null)
                    },
                ) { change, _ ->
                    drag = (change.position.x / width).coerceIn(0f, 1f)
                    dragCb(drag)
                }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Box(Modifier.fillMaxWidth().height(4.dp).clip(CircleShape).background(c.fill2))
        Box(Modifier.fillMaxWidth(f).height(4.dp).clip(CircleShape).background(c.ink))
        val thumb = if (drag != null) 16.dp else 12.dp
        Box(
            Modifier
                .offset {
                    val px = thumb.toPx()
                    IntOffset((f * width - px / 2).coerceIn(0f, (width - px).coerceAtLeast(0f)).roundToInt(), 0)
                }
                .size(thumb)
                .clip(CircleShape)
                .background(c.ink),
        )
    }
}

@Composable
fun EmptyState(icon: ImageVector, title: String, subtitle: String, action: (@Composable () -> Unit)? = null) {
    val c = VivelyTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Box(
            Modifier.size(52.dp).clip(CircleShape).background(c.fill).border(1.dp, c.line, CircleShape),
            contentAlignment = Alignment.Center,
        ) { VIcon(icon, 24.dp, c.ink2) }
        Spacer(Modifier.height(4.dp))
        VText(title, size = 16.sp, weight = FontWeight.SemiBold, align = TextAlign.Center)
        VText(subtitle, Modifier.widthIn(max = 380.dp), color = c.ink2, size = 13.5.sp, align = TextAlign.Center, lineHeight = 19.sp)
        if (action != null) {
            Spacer(Modifier.height(6.dp))
            action()
        }
    }
}

@Composable
fun ErrorState(title: String, detail: String, onRetry: () -> Unit) {
    val c = VivelyTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 40.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        VText(title, size = 15.5.sp, weight = FontWeight.SemiBold, align = TextAlign.Center)
        VText(detail, Modifier.widthIn(max = 400.dp), color = c.ink2, size = 13.sp, align = TextAlign.Center, lineHeight = 19.sp)
        Spacer(Modifier.height(6.dp))
        Pill("Повторить", height = 36.dp, onClick = onRetry)
    }
}

@Composable
fun Pulse(content: @Composable () -> Unit) {
    val t = rememberInfiniteTransition(label = "pulse")
    val a by t.animateFloat(1f, 0.45f, infiniteRepeatable(tween(900), RepeatMode.Reverse), label = "alpha")
    Box(Modifier.graphicsLayer { alpha = a }) { content() }
}

@Composable
fun Skel(w: Dp, h: Dp, modifier: Modifier = Modifier, radius: Dp = 6.dp) {
    Box(modifier.size(w, h).clip(RoundedCornerShape(radius)).background(VivelyTheme.colors.fill))
}

private val TITLE_W = intArrayOf(190, 240, 155, 215, 175, 255, 165)
private val ARTIST_W = intArrayOf(115, 150, 95, 130, 110, 125, 100)

@Composable
fun TrackListSkeleton(rows: Int = 8) {
    Pulse {
        Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
            repeat(rows) { i ->
                Row(Modifier.fillMaxWidth().height(64.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Skel(48.dp, 48.dp)
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(7.dp)) {
                        Skel(TITLE_W[i % 7].dp * 0.8f, 11.dp)
                        Skel(ARTIST_W[i % 7].dp * 0.8f, 9.dp)
                    }
                    Skel(30.dp, 9.dp)
                }
            }
        }
    }
}

@Composable
fun ShelfSkeleton() {
    Pulse {
        Column(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Skel(160.dp, 18.dp)
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                repeat(3) {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Skel(CARD_W, CARD_W, radius = 8.dp)
                        Skel(110.dp, 11.dp)
                        Skel(80.dp, 9.dp)
                    }
                }
            }
        }
    }
}

const val CARD_W_DP = 150
val CARD_W: Dp = CARD_W_DP.dp

@Composable
fun SectionHeader(
    title: String,
    subtitle: String? = null,
    action: String = "Все",
    topPadding: Dp = 28.dp,
    onAction: (() -> Unit)? = null,
) {
    val c = VivelyTheme.colors
    Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 6.dp, top = topPadding, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f).padding(end = 10.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            VText(title, size = 19.sp, weight = FontWeight.Bold, maxLines = 2, lineHeight = 23.sp, tracking = (-0.2).sp)
            if (!subtitle.isNullOrBlank()) VText(subtitle, color = c.ink2, size = 13.5.sp, maxLines = 2)
        }
        if (onAction != null) {
            VText(
                action,
                Modifier.tap(shape = CircleShape, onClick = onAction).padding(horizontal = 10.dp, vertical = 6.dp),
                color = c.ink2,
                size = 14.sp,
                weight = FontWeight.SemiBold,
                maxLines = 1,
            )
        }
    }
}

/** Top bar of pushed pages: back, a title that fades in on scroll, actions. */
@Composable
fun TopBar(
    title: String,
    titleVisible: Boolean,
    onBack: (() -> Unit)?,
    modifier: Modifier = Modifier,
    background: Color = VivelyTheme.colors.bg,
    actions: @Composable RowScope.() -> Unit = {},
) {
    BarRow(title, if (titleVisible) 1f else 0f, onBack, modifier.background(background), actions)
}

/** Top bar drawn over a hero: transparent at rest, filled with [color] as [fade] reaches 1. */
@Composable
fun FadingTopBar(
    title: String,
    fade: Float,
    color: Color,
    onBack: (() -> Unit)?,
    actions: @Composable RowScope.() -> Unit = {},
) {
    BarRow(
        title,
        ((fade - 0.6f) / 0.4f).coerceIn(0f, 1f),
        onBack,
        Modifier.background(color.copy(alpha = color.alpha * fade)),
        actions,
    )
}

@Composable
private fun BarRow(
    title: String,
    titleAlpha: Float,
    onBack: (() -> Unit)?,
    modifier: Modifier,
    actions: @Composable RowScope.() -> Unit,
) {
    Row(
        modifier.fillMaxWidth().statusBarsPadding().height(56.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (onBack != null) IconBtn(VIcons.ChevLeft, onBack, size = 44.dp, iconSize = 24.dp, tint = VivelyTheme.colors.ink) else Spacer(Modifier.width(12.dp))
        VText(
            title,
            Modifier.weight(1f).padding(horizontal = 4.dp).alpha(titleAlpha),
            size = 17.sp,
            weight = FontWeight.Bold,
            maxLines = 1,
        )
        actions()
    }
}

/** Large title of a tab root. */
@Composable
fun PageTitle(title: String, subtitle: String? = null, statusBar: Boolean = true, trailing: @Composable RowScope.() -> Unit = {}) {
    val c = VivelyTheme.colors
    val topMod = if (statusBar) Modifier.fillMaxWidth().statusBarsPadding() else Modifier.fillMaxWidth()
    Row(topMod.padding(start = 16.dp, end = 8.dp, top = 14.dp, bottom = 10.dp), verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            VText(title, size = 24.sp, weight = FontWeight.Bold, maxLines = 1, lineHeight = 29.sp, tracking = (-0.3).sp)
            if (subtitle != null) VText(subtitle, Modifier.padding(top = 2.dp), color = c.ink2, size = 14.sp, maxLines = 2)
        }
        trailing()
    }
}

@Composable
fun MenuRow(icon: ImageVector, label: String, sub: String? = null, danger: Boolean = false, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        Modifier.fillMaxWidth().tap(shape = androidx.compose.ui.graphics.RectangleShape, onClick = onClick).padding(horizontal = 16.dp, vertical = 13.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        VIcon(icon, 20.dp, if (danger) c.danger else c.ink)
        Column(Modifier.weight(1f)) {
            VText(label, color = if (danger) c.danger else c.ink, size = 15.sp, weight = FontWeight.Medium, maxLines = 1)
            if (sub != null) VText(sub, color = c.ink2, size = 13.5.sp, maxLines = 1)
        }
    }
}

/** Fades the content to transparent over [edge] at the top and bottom, whatever is behind it. */
fun Modifier.fadingEdges(edge: Dp): Modifier = this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
        drawContent()
        val e = edge.toPx().coerceAtMost(size.height / 2f)
        val h = size.height
        drawRect(
            Brush.verticalGradient(
                0f to Color.Transparent,
                e / h to Color.Black,
                (h - e) / h to Color.Black,
                1f to Color.Transparent,
            ),
            blendMode = BlendMode.DstIn,
        )
    }

@Composable
fun HLine(modifier: Modifier = Modifier) {
    Box(modifier.fillMaxWidth().height(1.dp).background(VivelyTheme.colors.line))
}

@Composable
fun PrefCard(title: String, sub: String?, right: @Composable () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        Modifier.fillMaxWidth().clip(RL).background(c.fill).padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Column(Modifier.weight(1f)) {
            VText(title, size = 14.5.sp, weight = FontWeight.SemiBold)
            if (sub != null) VText(sub, Modifier.padding(top = 2.dp), color = c.ink2, size = 13.sp, lineHeight = 17.sp)
        }
        right()
    }
}

@Composable
fun Wrap(modifier: Modifier = Modifier, spacing: Dp = 8.dp, content: @Composable () -> Unit) {
    FlowRow(
        modifier,
        horizontalArrangement = Arrangement.spacedBy(spacing),
        verticalArrangement = Arrangement.spacedBy(spacing),
    ) { content() }
}

@Composable
fun ColumnScope.SheetTitle(title: String, subtitle: String? = null) {
    val c = VivelyTheme.colors
    Column(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp)) {
        VText(title, size = 19.sp, weight = FontWeight.Bold)
        if (subtitle != null) VText(subtitle, Modifier.padding(top = 3.dp), color = c.ink2, size = 13.5.sp)
    }
}
