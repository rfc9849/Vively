package eu.r3pr3ss10n.vively

import android.Manifest
import android.content.ComponentName
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.background
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.ListenableFuture
import eu.r3pr3ss10n.vively.core.FfiAppearance
import eu.r3pr3ss10n.vively.playback.PlaybackService
import eu.r3pr3ss10n.vively.ui.AppShell
import eu.r3pr3ss10n.vively.ui.AuthScreen
import eu.r3pr3ss10n.vively.ui.CaptchaDialog
import eu.r3pr3ss10n.vively.ui.VivelyTheme
import eu.r3pr3ss10n.vively.ui.rememberArtworkTint

class MainActivity : ComponentActivity() {
    /** Binding a controller starts the session service; Media3 keeps it
     *  in the foreground while playing after the activity goes away. */
    private var controller: ListenableFuture<MediaController>? = null

    override fun onStart() {
        super.onStart()
        val token = SessionToken(this, ComponentName(this, PlaybackService::class.java))
        controller = MediaController.Builder(this, token).buildAsync()
    }

    override fun onStop() {
        controller?.let(MediaController::releaseFuture)
        controller = null
        super.onStop()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        // The media notification needs this on Android 13+.
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        val store = coreStore
        val prefs = themePrefs
        setContent {
            val pb by store.playback.collectAsState()
            val settings by store.settings.collectAsState()
            val dark = when (settings.appearance) {
                FfiAppearance.LIGHT -> false
                FfiAppearance.DARK -> true
                FfiAppearance.SYSTEM -> isSystemInDarkTheme()
            }
            LaunchedEffect(dark) {
                val t = android.graphics.Color.TRANSPARENT
                val style = if (dark) SystemBarStyle.dark(t) else SystemBarStyle.light(t, t)
                enableEdgeToEdge(style, style)
                if (Build.VERSION.SDK_INT >= 29) {
                    window.isStatusBarContrastEnforced = false
                    window.isNavigationBarContrastEnforced = false
                }
            }
            val activeCover = pb.current?.artworkUrl ?: pb.current?.thumbUrl
            val sampledTint = rememberArtworkTint(activeCover, pb.tintHex, keepPrevious = true)
            val systemTint = if (prefs.systemColors && ThemePrefs.supported) ThemePrefs.systemAccentHex(this) else null
            VivelyTheme(dark = dark, tintHex = systemTint ?: sampledTint) {
                VivelyRoot(store)
            }
        }
    }
}

@Composable
private fun VivelyRoot(store: CoreStore) {
    val auth by store.auth.collectAsState()
    val captcha by store.captcha.collectAsState()
    Box(Modifier.fillMaxSize().background(VivelyTheme.colors.bg)) {
        if (auth.signedIn) AppShell(store) else AuthScreen(store, auth)
    }
    captcha?.let { CaptchaDialog(it, onAnswer = store::answerCaptcha) }
}
