package eu.r3pr3ss10n.vively.playback

import android.content.Context
import android.content.Intent
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.session.MediaButtonReceiver
import eu.r3pr3ss10n.vively.coreStore

@OptIn(UnstableApi::class)
class PlaybackButtonReceiver : MediaButtonReceiver() {
    override fun shouldStartForegroundService(context: Context, intent: Intent): Boolean {
        val playback = context.coreStore.playback.value
        return playback.current != null && !playback.playing && !playback.loading
    }
}
