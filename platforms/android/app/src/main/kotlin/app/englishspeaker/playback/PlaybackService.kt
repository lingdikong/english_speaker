package app.englishspeaker.playback

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Looper
import android.view.KeyEvent
import androidx.core.content.IntentCompat
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import app.englishspeaker.MainActivity
import app.englishspeaker.SpeakerApp
import app.englishspeaker.core.PlayerStatus
import app.englishspeaker.data.SpeakerRepository
import app.englishspeaker.ui.cover.CoverRenderer
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * 后台播放 + 锁屏/通知控件。声音由核心层经 AudioTrackSink 输出，
 * 这里的 Player 只是把核心播放状态“投影”给 MediaSession。
 * 上一首/下一首 → 上一句/下一句（自定义命令按钮放在 back/forward 槽位，耳机按键同样映射）。
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val repo = SpeakerApp.repo
        val player = CorePlayer(this, repo)
        val prev = CommandButton.Builder(CommandButton.ICON_PREVIOUS)
            .setDisplayName("上一句")
            .setSessionCommand(SessionCommand(ACTION_PREV, Bundle.EMPTY))
            .setSlots(CommandButton.SLOT_BACK)
            .build()
        val next = CommandButton.Builder(CommandButton.ICON_NEXT)
            .setDisplayName("下一句")
            .setSessionCommand(SessionCommand(ACTION_NEXT, Bundle.EMPTY))
            .setSlots(CommandButton.SLOT_FORWARD)
            .build()
        val open = PendingIntent.getActivity(
            this, 0,
            Intent(this, MainActivity::class.java).putExtra("screen", "nowPlaying")
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        session = MediaSession.Builder(this, player)
            .setSessionActivity(open)
            .setMediaButtonPreferences(listOf(prev, next))
            .setCallback(object : MediaSession.Callback {
                override fun onConnect(s: MediaSession, c: MediaSession.ControllerInfo): MediaSession.ConnectionResult {
                    val cmds = MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
                        .add(SessionCommand(ACTION_PREV, Bundle.EMPTY))
                        .add(SessionCommand(ACTION_NEXT, Bundle.EMPTY))
                        .build()
                    return MediaSession.ConnectionResult.AcceptedResultBuilder(s)
                        .setAvailableSessionCommands(cmds)
                        .build()
                }

                override fun onCustomCommand(
                    s: MediaSession, c: MediaSession.ControllerInfo, command: SessionCommand, args: Bundle,
                ): ListenableFuture<SessionResult> {
                    when (command.customAction) {
                        ACTION_PREV -> repo.previousSentence()
                        ACTION_NEXT -> repo.nextSentence()
                    }
                    return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
                }

                override fun onMediaButtonEvent(s: MediaSession, c: MediaSession.ControllerInfo, intent: Intent): Boolean {
                    val ev = IntentCompat.getParcelableExtra(intent, Intent.EXTRA_KEY_EVENT, KeyEvent::class.java)
                    if (ev?.action != KeyEvent.ACTION_DOWN) return false
                    return when (ev.keyCode) {
                        KeyEvent.KEYCODE_MEDIA_NEXT -> { repo.nextSentence(); true }
                        KeyEvent.KEYCODE_MEDIA_PREVIOUS -> { repo.previousSentence(); true }
                        else -> false
                    }
                }
            })
            .build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        val p = session?.player
        if (p == null || !p.playWhenReady) stopSelf()
    }

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }

    companion object {
        const val ACTION_PREV = "app.englishspeaker.PREV_SENTENCE"
        const val ACTION_NEXT = "app.englishspeaker.NEXT_SENTENCE"
    }
}

/** 把核心播放状态投影成 Media3 Player。 */
private class CorePlayer(
    private val context: Context,
    private val repo: SpeakerRepository,
) : SimpleBasePlayer(Looper.getMainLooper()) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
    private val artwork = HashMap<String, ByteArray>()

    init {
        scope.launch {
            combine(repo.player, repo.articles) { p, list ->
                val a = list.firstOrNull { it.id == p.articleId }
                listOf(p.status, p.articleId, p.durationMs, p.rate, a?.title, a?.voiceId, p.positionMs / 5000u)
            }.distinctUntilChanged().collect {
                ensureArtwork()
                invalidateState()
            }
        }
        // 跳句等跳变时同步进度（正常播放由 Media3 按倍速外推）
        scope.launch {
            repo.position.map { it.sentence }.distinctUntilChanged().collect { invalidateState() }
        }
    }

    private fun ensureArtwork() {
        val a = repo.article(repo.player.value.articleId) ?: return
        if (artwork.containsKey(a.id)) return
        scope.launch {
            val bytes = withContext(Dispatchers.Default) { CoverRenderer.png(context, a.id, a.title) }
            artwork[a.id] = bytes
            invalidateState()
        }
    }

    override fun getState(): State {
        val ps = repo.player.value
        val a = repo.article(ps.articleId)
        val commands = Player.Commands.Builder().addAll(
            COMMAND_PLAY_PAUSE, COMMAND_STOP, COMMAND_SEEK_IN_CURRENT_MEDIA_ITEM,
            COMMAND_GET_CURRENT_MEDIA_ITEM, COMMAND_GET_METADATA, COMMAND_GET_TIMELINE,
            COMMAND_SET_SPEED_AND_PITCH,
        ).build()
        val b = State.Builder().setAvailableCommands(commands)
        if (a == null || ps.status == PlayerStatus.IDLE) {
            return b.setPlaybackState(STATE_IDLE).setPlayWhenReady(false, PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST).build()
        }
        val meta = MediaMetadata.Builder()
            .setTitle(a.title)
            .setArtist(repo.subtitle(a))
            .setAlbumTitle("English Speaker")
            .apply { artwork[a.id]?.let { setArtworkData(it, MediaMetadata.PICTURE_TYPE_FRONT_COVER) } }
            .build()
        val item = MediaItemData.Builder(a.id)
            .setMediaItem(MediaItem.Builder().setMediaId(a.id).setMediaMetadata(meta).build())
            .setMediaMetadata(meta)
            .setDurationUs(ps.durationMs.toLong() * 1000)
            .setIsSeekable(true)
            .build()
        val active = ps.status == PlayerStatus.PLAYING || ps.status == PlayerStatus.BUFFERING
        return b.setPlaylist(listOf(item))
            .setCurrentMediaItemIndex(0)
            .setPlaybackState(
                when (ps.status) {
                    PlayerStatus.BUFFERING -> STATE_BUFFERING
                    PlayerStatus.ENDED -> STATE_ENDED
                    else -> STATE_READY
                }
            )
            .setPlayWhenReady(active, PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST)
            .setContentPositionMs(repo.livePosition())
            .setPlaybackParameters(PlaybackParameters(ps.rate))
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (playWhenReady) {
            if (repo.player.value.status == PlayerStatus.IDLE) repo.toggle() else repo.play()
        } else repo.pause()
        return Futures.immediateVoidFuture()
    }

    override fun handleSeek(mediaItemIndex: Int, positionMs: Long, seekCommand: Int): ListenableFuture<*> {
        when (seekCommand) {
            COMMAND_SEEK_TO_NEXT, COMMAND_SEEK_TO_NEXT_MEDIA_ITEM -> repo.nextSentence()
            COMMAND_SEEK_TO_PREVIOUS, COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM -> repo.previousSentence()
            else -> repo.seek(positionMs)
        }
        return Futures.immediateVoidFuture()
    }

    override fun handleSetPlaybackParameters(playbackParameters: PlaybackParameters): ListenableFuture<*> {
        repo.setRate(playbackParameters.speed)
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        repo.pause()
        return Futures.immediateVoidFuture()
    }

    override fun handleRelease(): ListenableFuture<*> {
        scope.cancel()
        return Futures.immediateVoidFuture()
    }
}
