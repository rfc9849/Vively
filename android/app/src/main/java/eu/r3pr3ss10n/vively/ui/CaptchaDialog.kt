package eu.r3pr3ss10n.vively.ui

import android.annotation.SuppressLint
import android.graphics.Bitmap
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import coil3.compose.AsyncImage
import eu.r3pr3ss10n.vively.core.FfiCaptchaNav
import eu.r3pr3ss10n.vively.core.FfiEvent
import eu.r3pr3ss10n.vively.core.captchaInterceptorScript
import eu.r3pr3ss10n.vively.core.extractCaptchaSuccessToken
import eu.r3pr3ss10n.vively.core.parseCaptchaNav
import java.util.concurrent.atomic.AtomicBoolean

@Composable
fun CaptchaDialog(captcha: FfiEvent.Captcha, onAnswer: (ULong, String?) -> Unit) {
    val c = VivelyTheme.colors
    Dialog(
        onDismissRequest = { onAnswer(captcha.id, null) },
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Box(
            Modifier
                .fillMaxWidth(0.95f)
                .fillMaxHeight(0.8f)
                .clip(RL)
                .background(c.raised),
        ) {
            val url = captcha.redirectUrl
            if (url != null) {
                CaptchaWebView(url) { token -> onAnswer(captcha.id, token) }
            } else {
                ImageCaptcha(captcha.imageUrl) { answer -> onAnswer(captcha.id, answer) }
            }
        }
    }
}

/** Receives `window.ipc.postMessage(...)` from the injected interceptor script. */
private class CaptchaIpc(private val onToken: (String) -> Unit) {
    @JavascriptInterface
    fun postMessage(text: String) {
        extractCaptchaSuccessToken(text)?.let(onToken)
    }
}

@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun CaptchaWebView(url: String, onResult: (String?) -> Unit) {
    AndroidView(
        modifier = Modifier.fillMaxSize(),
        factory = { ctx ->
            WebView(ctx).apply {
                val answered = AtomicBoolean(false)
                val finish: (String?) -> Unit = { token ->
                    if (answered.compareAndSet(false, true)) post { onResult(token) }
                }
                settings.javaScriptEnabled = true
                settings.domStorageEnabled = true
                addJavascriptInterface(CaptchaIpc { finish(it) }, "ipc")
                webViewClient = object : WebViewClient() {
                    override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) {
                        view.evaluateJavascript(captchaInterceptorScript(), null)
                    }

                    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                        when (val nav = parseCaptchaNav(request.url.toString())) {
                            is FfiCaptchaNav.Success -> nav.token?.let {
                                finish(it)
                                return true
                            }
                            is FfiCaptchaNav.Cancel -> {
                                finish(null)
                                return true
                            }
                            null -> {}
                        }
                        val scheme = request.url.scheme
                        return scheme != "http" && scheme != "https"
                    }
                }
                loadUrl(url)
            }
        },
        onRelease = { it.destroy() },
    )
}

@Composable
private fun ImageCaptcha(imageUrl: String?, onSubmit: (String) -> Unit) {
    val c = VivelyTheme.colors
    var code by remember { mutableStateOf("") }
    Column(
        Modifier.fillMaxSize().padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        VText("Введите символы с картинки")
        AsyncImage(
            model = imageUrl,
            contentDescription = null,
            contentScale = ContentScale.Fit,
            modifier = Modifier.fillMaxWidth().height(120.dp).background(c.fill),
        )
        VField(code, { code = it }, "Код с картинки", onDone = { if (code.isNotBlank()) onSubmit(code.trim()) })
        PrimaryPill(null, "Отправить", Modifier.fillMaxWidth()) { if (code.isNotBlank()) onSubmit(code.trim()) }
    }
}
