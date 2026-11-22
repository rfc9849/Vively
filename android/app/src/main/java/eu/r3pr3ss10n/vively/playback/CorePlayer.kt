package eu.r3pr3ss10n.vively.playback

import android.net.Uri
import android.os.Looper
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import eu.r3pr3ss10n.vively.core.FfiPlayback
import eu.r3pr3ss10n.vively.core.FfiRepeat
import eu.r3pr3ss10n.vively.core.FfiTrack
import eu.r3pr3ss10n.vively.core.VivelyCore

/**
 * Media3 view of the Rust player. State comes from core snapshots; commands
 * from the notification, lock screen, Bluetooth and Android Auto go back to
 * the core. Nothing here decodes audio.
 */
class CorePlayer(
    looper: Looper,
    private val core: VivelyCore,
    private var snapshot: FfiPlayback,
) : SimpleBasePlayer(looper) {

    /** Call on the player's looper (main). */
    fun update(pb: FfiPlayback) {
        if (pb == snapshot) return
        snapshot = pb
        invalidateState()
    }

    override fun getState(): State {
        val pb = snapshot
        val builder = State.Builder()
            .setAvailableCommands(COMMANDS)
            .setAudioAttributes(MUSIC)
            .setShuffleModeEnabled(pb.shuffled)
            .setRepeatMode(
                when (pb.repeat) {
                    FfiRepeat.OFF -> Player.REPEAT_MODE_OFF
                    FfiRepeat.ALL -> Player.REPEAT_MODE_ALL
                    FfiRepeat.ONE -> Player.REPEAT_MODE_ONE
                },
            )
        val current = pb.current
            ?: return builder
                .setPlaybackState(Player.STATE_IDLE)
                .setPlayWhenReady(false, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
                .build()

        val items = ArrayList<MediaItemData>(pb.upcoming.size + 1)
        items += item(0, current)
        pb.upcoming.forEachIndexed { i, t -> items += item(i + 1, t) }
        val posMs = (pb.positionSecs * 1000).toLong().coerceAtLeast(0)
        return builder
            .setPlaylist(items)
            .setCurrentMediaItemIndex(0)
            .setPlaybackState(if (pb.loading) Player.STATE_BUFFERING else Player.STATE_READY)
            .setPlayWhenReady(pb.playing || pb.loading, Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setContentPositionMs(
                if (pb.playing) PositionSupplier.getExtrapolating(posMs, 1f)
                else PositionSupplier.getConstant(posMs),
            )
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (playWhenReady) core.play() else core.pause()
        return done()
    }

    override fun handleStop(): ListenableFuture<*> {
        core.stop()
        return done()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            Player.COMMAND_SEEK_TO_NEXT, Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> core.nextTrack()
            Player.COMMAND_SEEK_TO_PREVIOUS, Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> core.prevTrack()
            Player.COMMAND_SEEK_TO_MEDIA_ITEM, Player.COMMAND_SEEK_TO_DEFAULT_POSITION -> {
                val target = snapshot.upcoming.getOrNull(mediaItemIndex - 1)
                if (mediaItemIndex > 0 && target != null) {
                    core.selectQueueTrack(target.fullId)
                } else {
                    core.seekTo(if (positionMs == C.TIME_UNSET) 0.0 else positionMs / 1000.0)
                }
            }
            else -> core.seekTo(positionMs.coerceAtLeast(0) / 1000.0)
        }
        return done()
    }

    override fun handleSetRepeatMode(repeatMode: Int): ListenableFuture<*> {
        core.setRepeat(
            when (repeatMode) {
                Player.REPEAT_MODE_ONE -> FfiRepeat.ONE
                Player.REPEAT_MODE_ALL -> FfiRepeat.ALL
                else -> FfiRepeat.OFF
            },
        )
        return done()
    }

    override fun handleSetShuffleModeEnabled(shuffleModeEnabled: Boolean): ListenableFuture<*> {
        core.setShuffle(shuffleModeEnabled)
        return done()
    }

    override fun handleRelease(): ListenableFuture<*> = done()

    private fun done(): ListenableFuture<*> = Futures.immediateVoidFuture()

    private fun item(index: Int, t: FfiTrack): MediaItemData =
        // Uids must be unique; the same track can sit in the queue twice.
        MediaItemData.Builder("$index:${t.fullId}")
            .setMediaItem(
                MediaItem.Builder()
                    .setMediaId(t.fullId)
                    .setMediaMetadata(
                        MediaMetadata.Builder()
                            .setTitle(t.title)
                            .setArtist(t.artist)
                            .setArtworkUri(t.artworkUrl?.let(Uri::parse))
                            .setIsPlayable(t.playable)
                            .setIsBrowsable(false)
                            .build(),
                    )
                    .build(),
            )
            .setDurationUs(t.durationSecs * 1_000_000L)
            .setIsSeekable(true)
            .build()

    private companion object {
        val MUSIC: AudioAttributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()

        val COMMANDS: Player.Commands = Player.Commands.Builder()
            .addAll(
                Player.COMMAND_PLAY_PAUSE,
                Player.COMMAND_STOP,
                Player.COMMAND_SEEK_TO_NEXT,
                Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_PREVIOUS,
                Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
                Player.COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_MEDIA_ITEM,
                Player.COMMAND_SEEK_TO_DEFAULT_POSITION,
                Player.COMMAND_SET_REPEAT_MODE,
                Player.COMMAND_SET_SHUFFLE_MODE,
                Player.COMMAND_GET_CURRENT_MEDIA_ITEM,
                Player.COMMAND_GET_TIMELINE,
                Player.COMMAND_GET_METADATA,
                Player.COMMAND_GET_AUDIO_ATTRIBUTES,
                Player.COMMAND_RELEASE,
            )
            .build()
    }
}
