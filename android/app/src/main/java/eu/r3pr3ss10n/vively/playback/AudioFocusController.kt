package eu.r3pr3ss10n.vively.playback

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import eu.r3pr3ss10n.vively.core.VivelyCore

/**
 * Audio focus and "becoming noisy" (headphones unplugged) for the Rust
 * player, which outputs through AAudio and knows nothing about either.
 * All calls happen on the main thread.
 */
class AudioFocusController(private val context: Context, private val core: VivelyCore) {
    private val am = context.getSystemService(AudioManager::class.java)
    private var active = false
    private var hasFocus = false
    private var resumeOnGain = false
    private var noisyRegistered = false

    private val noisy = object : BroadcastReceiver() {
        override fun onReceive(c: Context, intent: Intent) {
            if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) core.pause()
        }
    }

    private val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
        .setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_MEDIA)
                .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                .build(),
        )
        .setOnAudioFocusChangeListener(::onFocusChange, Handler(Looper.getMainLooper()))
        .build()

    /** `playing` = playing or about to (loading with play intent). */
    fun onPlayback(playing: Boolean) {
        if (playing == active) return
        active = playing
        if (playing) {
            resumeOnGain = false
            if (!hasFocus) {
                when (am.requestAudioFocus(request)) {
                    AudioManager.AUDIOFOCUS_REQUEST_GRANTED -> hasFocus = true
                    AudioManager.AUDIOFOCUS_REQUEST_DELAYED -> resumeOnGain = true
                    else -> core.pause()
                }
            }
            registerNoisy()
        } else {
            unregisterNoisy()
            // Keep the request alive across a transient loss so GAIN arrives.
            if (!resumeOnGain) abandon()
        }
    }

    fun release() {
        unregisterNoisy()
        abandon()
    }

    private fun onFocusChange(change: Int) {
        when (change) {
            AudioManager.AUDIOFOCUS_GAIN -> {
                hasFocus = true
                if (resumeOnGain) {
                    resumeOnGain = false
                    core.play()
                }
            }
            AudioManager.AUDIOFOCUS_LOSS -> {
                resumeOnGain = false
                core.pause()
                abandon()
            }
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
                hasFocus = false
                resumeOnGain = active
                core.pause()
            }
            // Android 8+ ducks automatically for apps that don't opt out.
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {}
        }
    }

    private fun abandon() {
        am.abandonAudioFocusRequest(request)
        hasFocus = false
    }

    private fun registerNoisy() {
        if (noisyRegistered) return
        val filter = IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY)
        if (Build.VERSION.SDK_INT >= 33) {
            context.registerReceiver(noisy, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(noisy, filter)
        }
        noisyRegistered = true
    }

    private fun unregisterNoisy() {
        if (!noisyRegistered) return
        context.unregisterReceiver(noisy)
        noisyRegistered = false
    }
}
