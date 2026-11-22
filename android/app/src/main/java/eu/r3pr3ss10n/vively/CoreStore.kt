package eu.r3pr3ss10n.vively

import android.app.Application
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.Build
import eu.r3pr3ss10n.vively.core.CoreListener
import eu.r3pr3ss10n.vively.core.FfiAuth
import eu.r3pr3ss10n.vively.core.FfiConfig
import eu.r3pr3ss10n.vively.core.FfiDirty
import eu.r3pr3ss10n.vively.core.FfiEvent
import eu.r3pr3ss10n.vively.core.FfiLibrary
import eu.r3pr3ss10n.vively.core.FfiNetwork
import eu.r3pr3ss10n.vively.core.FfiNetworkStatus
import eu.r3pr3ss10n.vively.core.FfiPlayback
import eu.r3pr3ss10n.vively.core.FfiSettings
import eu.r3pr3ss10n.vively.core.VivelyCore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import java.io.File

/**
 * Owns the Rust core for the whole process and mirrors its state into flows.
 *
 * The core calls [CoreListener.onUpdate] on its own thread, where calling back
 * into the core would deadlock, so updates are queued and snapshots are pulled
 * on [Dispatchers.Default]. Big screens watch a revision counter and re-query
 * their snapshot when it moves.
 */
class CoreStore(private val app: Application) {
    val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val updates = Channel<Pair<FfiDirty, List<FfiEvent>>>(Channel.UNLIMITED)

    val core: VivelyCore = VivelyCore(
        FfiConfig(
            configDir = File(app.filesDir, "config").absolutePath,
            dataDir = File(app.filesDir, "data").absolutePath,
            deviceName = "Vively (${Build.MODEL})",
            simulatedAudio = false,
            apiHost = null,
        ),
        Listener(updates, NetworkProbe(app)),
    )

    private val _auth = MutableStateFlow(core.auth())
    private val _library = MutableStateFlow(core.library())
    private val _playback = MutableStateFlow(core.playback())
    private val _settings = MutableStateFlow(core.settings())
    private val _captcha = MutableStateFlow<FfiEvent.Captcha?>(null)
    private val _events = MutableSharedFlow<FfiEvent>(extraBufferCapacity = 64)

    private val _homeRev = MutableStateFlow(0)
    private val _searchRev = MutableStateFlow(0)
    private val _playlistsRev = MutableStateFlow(0)
    private val _pagesRev = MutableStateFlow(0)
    private val _friendsRev = MutableStateFlow(0)
    private val _downloadsRev = MutableStateFlow(0)
    private val _lyricsRev = MutableStateFlow(0)
    private val _editorRev = MutableStateFlow(0)
    private val _uploadRev = MutableStateFlow(0)

    val auth: StateFlow<FfiAuth> = _auth
    val library: StateFlow<FfiLibrary> = _library
    val playback: StateFlow<FfiPlayback> = _playback
    val settings: StateFlow<FfiSettings> = _settings
    /** Pending captcha; kept as state so it survives the UI not listening. */
    val captcha: StateFlow<FfiEvent.Captcha?> = _captcha
    /** One-shot events, toasts included. */
    val events: SharedFlow<FfiEvent> = _events

    val homeRev: StateFlow<Int> = _homeRev
    val searchRev: StateFlow<Int> = _searchRev
    val playlistsRev: StateFlow<Int> = _playlistsRev
    val pagesRev: StateFlow<Int> = _pagesRev
    val friendsRev: StateFlow<Int> = _friendsRev
    val downloadsRev: StateFlow<Int> = _downloadsRev
    val lyricsRev: StateFlow<Int> = _lyricsRev
    val editorRev: StateFlow<Int> = _editorRev
    val uploadRev: StateFlow<Int> = _uploadRev

    init {
        scope.launch {
            for ((dirty, events) in updates) {
                if (dirty.auth || dirty.session) _auth.value = core.auth()
                if (dirty.library || dirty.downloads || dirty.session) _library.value = core.library()
                if (dirty.playback || dirty.position || dirty.library || dirty.downloads) _playback.value = core.playback()
                if (dirty.settings || dirty.session) _settings.value = core.settings()
                // Track rows carry liked/downloaded flags, so these touch every list.
                val lists = dirty.library || dirty.session || dirty.downloads
                if (dirty.home || dirty.playback || lists) _homeRev.bump()
                if (dirty.search || lists) _searchRev.bump()
                if (dirty.playlists || lists) _playlistsRev.bump()
                if (dirty.pages || dirty.playlists || lists) _pagesRev.bump()
                if (dirty.friends || lists) _friendsRev.bump()
                if (lists) _downloadsRev.bump()
                if (dirty.lyrics || dirty.playback) _lyricsRev.bump()
                if (dirty.editor || lists) _editorRev.bump()
                if (dirty.upload) _uploadRev.bump()
                for (ev in events) {
                    if (ev is FfiEvent.Captcha) _captcha.value = ev else _events.emit(ev)
                }
            }
        }
    }

    private fun MutableStateFlow<Int>.bump() {
        value += 1
    }

    fun answerCaptcha(id: ULong, answer: String?) {
        if (_captcha.value?.id == id) _captcha.value = null
        core.answerCaptcha(id, answer)
    }

    private class Listener(
        private val updates: Channel<Pair<FfiDirty, List<FfiEvent>>>,
        private val network: NetworkProbe,
    ) : CoreListener {
        override fun onUpdate(dirty: FfiDirty, events: List<FfiEvent>) {
            updates.trySend(dirty to events)
        }

        override fun networkState(): FfiNetwork = network.current()
    }
}

private class NetworkProbe(app: Application) {
    private val cm = app.getSystemService(ConnectivityManager::class.java)

    fun current(): FfiNetwork {
        val caps = cm.activeNetwork?.let { cm.getNetworkCapabilities(it) }
            ?: return FfiNetwork(FfiNetworkStatus.OFFLINE, expensive = false, constrained = false)
        val status = when {
            caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) -> FfiNetworkStatus.WIFI
            caps.hasTransport(NetworkCapabilities.TRANSPORT_CELLULAR) -> FfiNetworkStatus.CELLULAR
            caps.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET) -> FfiNetworkStatus.WIRED
            else -> FfiNetworkStatus.OTHER
        }
        return FfiNetwork(
            status = status,
            expensive = !caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED),
            constrained = cm.restrictBackgroundStatus ==
                ConnectivityManager.RESTRICT_BACKGROUND_STATUS_ENABLED,
        )
    }
}
