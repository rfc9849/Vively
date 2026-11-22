package eu.r3pr3ss10n.vively.ui

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import eu.r3pr3ss10n.vively.CoreStore
import eu.r3pr3ss10n.vively.core.FfiAuth
import eu.r3pr3ss10n.vively.core.FfiAuthStep
import eu.r3pr3ss10n.vively.core.FfiEvent

@Composable
fun AuthScreen(store: CoreStore, auth: FfiAuth) {
    val c = VivelyTheme.colors
    val core = store.core
    var input by rememberSaveable { mutableStateOf("") }
    val focus = remember { FocusRequester() }

    LaunchedEffect(auth.step::class) {
        if (auth.step !is FfiAuthStep.Phone) input = ""
    }

    LaunchedEffect(store) {
        store.events.collect { ev ->
            if (ev is FfiEvent.FocusAuthField) {
                if (ev.clear) input = ""
                runCatching { focus.requestFocus() }
            }
        }
    }

    val heading = when (val s = auth.step) {
        FfiAuthStep.Phone -> "Добро пожаловать"
        is FfiAuthStep.Otp -> s.methodLabel
        is FfiAuthStep.Password -> "Пароль"
        else -> "Вход по QR-коду"
    }

    Box(
        Modifier
            .fillMaxSize()
            .background(c.bg)
            .statusBarsPadding()
            .imePadding()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 28.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .widthIn(max = 440.dp)
                .clip(RL)
                .background(c.raised)
                .border(1.dp, c.line, RL),
        ) {
            Row(
                Modifier.fillMaxWidth().padding(start = 24.dp, end = 20.dp, top = 22.dp, bottom = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Box(
                    Modifier.size(30.dp).clip(RoundedCornerShape(7.dp)).background(c.ink),
                    contentAlignment = Alignment.Center,
                ) { VIcon(VIcons.Logo, 19.dp, c.bg) }
                VText(heading, size = 19.sp, weight = FontWeight.Bold, lineHeight = 23.sp)
            }

            when (val step = auth.step) {
                FfiAuthStep.Phone -> AuthFormBody(
                    subtitle = "Войдите через ВКонтакте, чтобы слушать музыку",
                    placeholder = "Номер телефона",
                    value = input,
                    onChange = { input = it },
                    keyboardType = KeyboardType.Phone,
                    masked = false,
                    loading = auth.loading,
                    error = auth.error,
                    focus = focus,
                    onSubmit = { if (input.isNotBlank() && !auth.loading) core.submitAuth(input.trim()) },
                )
                is FfiAuthStep.Otp -> AuthFormBody(
                    subtitle = step.subtitle,
                    placeholder = step.placeholder,
                    value = input,
                    onChange = { input = it },
                    keyboardType = KeyboardType.Number,
                    masked = false,
                    loading = auth.loading,
                    error = auth.error,
                    focus = focus,
                    onSubmit = { if (input.isNotBlank() && !auth.loading) core.submitAuth(input.trim()) },
                    extraAction = {
                        if (step.resendLabel.isNotBlank()) {
                            VText(
                                step.resendLabel,
                                Modifier.tap(enabled = !auth.loading) { core.resendAuthOtp() }.padding(vertical = 6.dp, horizontal = 4.dp),
                                color = c.ink2,
                                size = 13.5.sp,
                                weight = FontWeight.SemiBold,
                            )
                        }
                    },
                )
                is FfiAuthStep.Password -> AuthFormBody(
                    subtitle = "Введите пароль от аккаунта ВКонтакте",
                    placeholder = "Пароль",
                    value = input,
                    onChange = { input = it },
                    keyboardType = KeyboardType.Password,
                    masked = true,
                    loading = auth.loading,
                    error = auth.error,
                    focus = focus,
                    onSubmit = { if (input.isNotEmpty() && !auth.loading) core.submitAuth(input) },
                )
                is FfiAuthStep.QrConfirmCode -> AuthFormBody(
                    subtitle = "Введите код подтверждения, который появился в приложении ВКонтакте",
                    placeholder = "Код подтверждения",
                    value = input,
                    onChange = { input = it },
                    keyboardType = KeyboardType.Text,
                    masked = false,
                    loading = auth.loading,
                    error = auth.error,
                    focus = focus,
                    onSubmit = { if (input.isNotBlank() && !auth.loading) core.submitAuth(input.trim()) },
                )
                FfiAuthStep.QrLoading -> QrStatusBody(
                    loading = true,
                    caption = "Генерация QR-кода...",
                    error = auth.error,
                )
                is FfiAuthStep.QrReady -> QrMatrixBody(
                    size = step.size.toInt(),
                    modules = step.modules,
                    caption = "Откройте приложение ВКонтакте на телефоне и наведите камеру на QR-код",
                    sub = "Ожидание сканирования...",
                    error = auth.error,
                )
                is FfiAuthStep.QrScanned -> QrMatrixBody(
                    size = step.size.toInt(),
                    modules = step.modules,
                    caption = step.userName?.let { "Подтвердите вход для $it в приложении ВКонтакте" }
                        ?: "Нажмите «Подтвердить» на телефоне, чтобы завершить вход",
                    sub = "Ожидание подтверждения...",
                    dimmed = true,
                    error = auth.error,
                )
                FfiAuthStep.QrExpired -> QrStatusBody(
                    loading = false,
                    caption = "Срок действия QR-кода истёк",
                    error = auth.error,
                )
                FfiAuthStep.QrDeclined -> QrStatusBody(
                    loading = false,
                    caption = "Вход был отклонён на телефоне",
                    error = auth.error,
                )
            }

            HLine()
            AuthFooter(
                auth = auth,
                input = input,
                onSubmit = {
                    val v = if (auth.step is FfiAuthStep.Password) input else input.trim()
                    if (v.isNotEmpty() && !auth.loading) core.submitAuth(v)
                },
                onStartQr = { core.startQrAuth() },
                onCancelQr = { core.cancelQrAuth() },
                onBack = { core.authGoBack() },
            )
        }
    }
}

@Composable
private fun AuthFormBody(
    subtitle: String,
    placeholder: String,
    value: String,
    onChange: (String) -> Unit,
    keyboardType: KeyboardType,
    masked: Boolean,
    loading: Boolean,
    error: String?,
    focus: FocusRequester,
    onSubmit: () -> Unit,
    extraAction: (@Composable () -> Unit)? = null,
) {
    val c = VivelyTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(start = 24.dp, end = 24.dp, top = 4.dp, bottom = 20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        VText(subtitle, color = c.ink2, size = 14.sp, lineHeight = 20.sp)
        VField(
            value = value,
            onChange = onChange,
            placeholder = placeholder,
            keyboardType = keyboardType,
            masked = masked,
            imeAction = ImeAction.Done,
            onDone = onSubmit,
            focusRequester = focus,
            enabled = !loading,
        )
        if (!error.isNullOrBlank()) {
            VText(error, color = c.danger, size = 13.sp, lineHeight = 18.sp)
        }
        extraAction?.invoke()
    }
}

@Composable
private fun QrStatusBody(loading: Boolean, caption: String, error: String?) {
    val c = VivelyTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 28.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        if (loading) {
            Spinner(28.dp, c.ink)
        } else {
            Box(
                Modifier.size(48.dp).clip(RL).background(c.fill),
                contentAlignment = Alignment.Center,
            ) { VIcon(VIcons.QrCode, 24.dp, c.ink2) }
        }
        VText(caption, color = c.ink2, size = 14.sp, align = TextAlign.Center, lineHeight = 20.sp)
        if (!error.isNullOrBlank()) {
            VText(error, color = c.danger, size = 13.sp, align = TextAlign.Center)
        }
    }
}

@Composable
private fun QrMatrixBody(
    size: Int,
    modules: List<Boolean>,
    caption: String,
    sub: String,
    dimmed: Boolean = false,
    error: String?,
) {
    val c = VivelyTheme.colors
    Column(
        Modifier.fillMaxWidth().padding(horizontal = 24.dp, vertical = 12.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        VText(caption, color = c.ink2, size = 14.sp, align = TextAlign.Center, lineHeight = 20.sp)
        Box(
            Modifier.size(216.dp).clip(RL).background(Color.White).padding(14.dp),
            contentAlignment = Alignment.Center,
        ) {
            if (size > 0 && modules.size >= size * size) {
                Canvas(Modifier.fillMaxSize()) {
                    val cell = this.size.width / size
                    val fg = if (dimmed) Color(0x55111111) else Color(0xFF111111)
                    for (y in 0 until size) {
                        val row = y * size
                        for (x in 0 until size) {
                            if (modules[row + x]) {
                                drawRect(
                                    color = fg,
                                    topLeft = Offset(x * cell, y * cell),
                                    size = Size(cell + 0.5f, cell + 0.5f),
                                )
                            }
                        }
                    }
                }
            }
            if (dimmed) Spinner(28.dp, Color(0xFF111111))
        }
        VText(sub, color = c.ink3, size = 13.sp, align = TextAlign.Center)
        if (!error.isNullOrBlank()) {
            VText(error, color = c.danger, size = 13.sp, align = TextAlign.Center)
        }
        Spacer(Modifier.height(4.dp))
    }
}

@Composable
private fun AuthFooter(
    auth: FfiAuth,
    input: String,
    onSubmit: () -> Unit,
    onStartQr: () -> Unit,
    onCancelQr: () -> Unit,
    onBack: () -> Unit,
) {
    val c = VivelyTheme.colors
    Row(
        Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        when (auth.step) {
            FfiAuthStep.Phone -> {
                Pill("Войти по QR", icon = VIcons.QrCode, enabled = !auth.loading, onClick = onStartQr)
                Spacer(Modifier.weight(1f))
                SubmitAuthPill(
                    label = if (auth.loading) "Подождите..." else "Продолжить",
                    loading = auth.loading,
                    enabled = input.isNotBlank() && !auth.loading,
                    onClick = onSubmit,
                )
            }
            is FfiAuthStep.Otp -> {
                Pill("Назад", enabled = !auth.loading, onClick = onBack)
                Spacer(Modifier.weight(1f))
                SubmitAuthPill(
                    label = if (auth.loading) "Подождите..." else "Подтвердить",
                    loading = auth.loading,
                    enabled = input.isNotBlank() && !auth.loading,
                    onClick = onSubmit,
                )
            }
            is FfiAuthStep.Password -> {
                Pill("Назад", enabled = !auth.loading, onClick = onBack)
                Spacer(Modifier.weight(1f))
                SubmitAuthPill(
                    label = if (auth.loading) "Подождите..." else "Войти",
                    loading = auth.loading,
                    enabled = input.isNotEmpty() && !auth.loading,
                    onClick = onSubmit,
                )
            }
            is FfiAuthStep.QrConfirmCode -> {
                Pill("Назад", enabled = !auth.loading, onClick = onCancelQr)
                Spacer(Modifier.weight(1f))
                SubmitAuthPill(
                    label = if (auth.loading) "Подождите..." else "Подтвердить",
                    loading = auth.loading,
                    enabled = input.isNotBlank() && !auth.loading,
                    onClick = onSubmit,
                )
            }
            FfiAuthStep.QrExpired, FfiAuthStep.QrDeclined -> {
                Pill("Назад", enabled = !auth.loading, onClick = onCancelQr)
                Spacer(Modifier.weight(1f))
                PrimaryPill(VIcons.QrCode, "Обновить QR-код", onClick = onStartQr)
            }
            FfiAuthStep.QrLoading, is FfiAuthStep.QrReady, is FfiAuthStep.QrScanned -> {
                Pill("Назад", onClick = onCancelQr)
                Spacer(Modifier.weight(1f))
                if (auth.loading) Spinner(18.dp, c.ink2)
            }
        }
    }
}

@Composable
private fun SubmitAuthPill(label: String, loading: Boolean, enabled: Boolean, onClick: () -> Unit) {
    val c = VivelyTheme.colors
    Row(
        Modifier
            .height(40.dp)
            .clip(RS)
            .background(if (enabled || loading) c.ctaBg else c.fill2)
            .tap(enabled = enabled, pressed = c.ctaHover, onClick = onClick)
            .padding(horizontal = 18.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        if (loading) Spinner(16.dp, c.ctaFg)
        VText(label, color = if (enabled || loading) c.ctaFg else c.ink3, size = 14.5.sp, weight = FontWeight.Bold)
    }
}
