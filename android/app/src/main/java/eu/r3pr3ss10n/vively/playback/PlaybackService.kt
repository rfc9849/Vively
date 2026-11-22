package eu.r3pr3ss10n.vively.playback

import android.app.PendingIntent
import android.content.Intent
import android.os.Looper
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import eu.r3pr3ss10n.vively.MainActivity
import eu.r3pr3ss10n.vively.coreStore
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch

/**
 * Keeps the process alive while music plays and publishes the media session
 * (notification, lock screen, Bluetooth, Android Auto). Media3 promotes the
 * service to foreground on its own while the player is playing.
 */
class PlaybackService : MediaSessionService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var session: MediaSession? = null
    private lateinit var focus: AudioFocusController

    override fun onCreate() {
        super.onCreate()
        val store = coreStore
        val player = CorePlayer(Looper.getMainLooper(), store.core, store.playback.value)
        focus = AudioFocusController(this, store.core)
        val openApp = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, player)
            .setSessionActivity(openApp)
            .build()
        scope.launch {
            store.playback.collect { pb ->
                player.update(pb)
                focus.onPlayback(pb.playing || pb.loading)
            }
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        scope.cancel()
        focus.release()
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }
}
