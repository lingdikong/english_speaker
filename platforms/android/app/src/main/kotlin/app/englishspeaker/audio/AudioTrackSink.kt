package app.englishspeaker.audio

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.media.PlaybackParams
import android.os.Build
import android.os.SystemClock
import android.util.Log
import app.englishspeaker.core.PlatformAudioSink

/**
 * 核心层的音频输出：AudioTrack（PCM_FLOAT、MODE_STREAM、阻塞写入）。
 *
 * - `write` 用 WRITE_BLOCKING：缓冲（约 200ms）写满时阻塞，天然满足核心层要求的背压语义；
 * - `playedFrames` 基于 `playbackHeadPosition`（32 位无符号、会回绕），flush/open 后归零；
 * - API < 31 无法设置起播阈值，缓冲未满时 AudioTrack 不会开始播放（短句尾巴会“卡住”），
 *   这时在饥饿后补写静音把真实数据推出去，并在帧数统计中扣除这些静音。
 *
 * 所有方法都在核心的 es-player 线程调用（`playedFrames` 也可能来自其他线程），用锁保护状态，
 * 但阻塞写入本身在锁外进行。
 */
class AudioTrackSink : PlatformAudioSink {
    private val lock = Any()
    private var track: AudioTrack? = null
    private var channels = 1
    private var bufferFrames = 0
    private var rate = 1f
    private var playing = false

    // —— 帧数统计（flush/open 后归零）——
    private var lastRawHead = 0L      // 上次读到的 32 位无符号原始值
    private var wraps = 0L            // 回绕次数
    private var streamWritten = 0L    // 写入 AudioTrack 的总帧数（含补写的静音）
    private val silences = ArrayList<LongArray>() // 补写静音区间 [start, end)（流坐标）
    private var lastHeadChange = 0L

    override fun openOutput(sampleRate: UInt, channels: UShort): Boolean = synchronized(lock) {
        releaseLocked()
        return try {
            val ch = channels.toInt().coerceIn(1, 2)
            val mask = if (ch == 1) AudioFormat.CHANNEL_OUT_MONO else AudioFormat.CHANNEL_OUT_STEREO
            val sr = sampleRate.toInt()
            val minBytes = AudioTrack.getMinBufferSize(sr, mask, AudioFormat.ENCODING_PCM_FLOAT)
            // 约 200ms 的缓冲
            val wantBytes = sr / 5 * ch * 4
            val bytes = maxOf(minBytes, wantBytes)
            val t = AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                        .setSampleRate(sr)
                        .setChannelMask(mask)
                        .build()
                )
                .setBufferSizeInBytes(bytes)
                .setTransferMode(AudioTrack.MODE_STREAM)
                .build()
            if (t.state != AudioTrack.STATE_INITIALIZED) {
                t.release()
                return false
            }
            if (Build.VERSION.SDK_INT >= 31) {
                // 有数据就开播，不必等缓冲写满
                runCatching { t.startThresholdInFrames = 1 }
            }
            this.channels = ch
            this.bufferFrames = t.bufferSizeInFrames
            track = t
            applyRateLocked()
            resetCountersLocked()
            // 核心约定：open 之后即处于播放态（autoplay=false 时会紧接着调用 pause）
            t.play()
            playing = true
            true
        } catch (e: Exception) {
            Log.e(TAG, "open failed", e)
            false
        }
    }

    override fun write(samples: List<Float>) {
        val t = synchronized(lock) { track } ?: return
        val data = samples.toFloatArray()
        var off = 0
        while (off < data.size) {
            val n = t.write(data, off, data.size - off, AudioTrack.WRITE_BLOCKING)
            if (n <= 0) break // 已 release / flush 中断
            off += n
        }
        synchronized(lock) {
            if (track === t) streamWritten += (off / channels).toLong()
        }
    }

    override fun pause() = synchronized(lock) {
        track?.let { runCatching { it.pause() } }
        playing = false
    }

    override fun resume() = synchronized(lock) {
        track?.let { runCatching { it.play() } }
        playing = true
        lastHeadChange = SystemClock.elapsedRealtime()
    }

    override fun flush() = synchronized(lock) {
        val t = track ?: return@synchronized
        // AudioTrack.flush 只在暂停/停止时生效
        runCatching {
            if (playing) t.pause()
            t.flush()
            if (playing) t.play()
        }
        resetCountersLocked()
    }

    override fun playedFrames(): ULong = synchronized(lock) {
        val t = track ?: return@synchronized 0uL
        val raw = t.playbackHeadPosition.toLong() and 0xFFFF_FFFFL
        if (raw < lastRawHead) wraps++
        val now = SystemClock.elapsedRealtime()
        if (raw != lastRawHead) lastHeadChange = now
        lastRawHead = raw
        val head = (wraps shl 32) + raw

        // 旧系统：缓冲未满不起播。饥饿超过 150ms 仍有未播数据时补写静音（非阻塞）。
        if (Build.VERSION.SDK_INT < 31 && playing && head < streamWritten &&
            now - lastHeadChange > 150
        ) {
            val pad = bufferFrames
            val buf = FloatArray(pad * channels)
            val n = t.write(buf, 0, buf.size, AudioTrack.WRITE_NON_BLOCKING)
            if (n > 0) {
                val frames = (n / channels).toLong()
                silences.add(longArrayOf(streamWritten, streamWritten + frames))
                streamWritten += frames
            }
            lastHeadChange = now
        }

        var silent = 0L
        for (s in silences) {
            if (head > s[0]) silent += minOf(head, s[1]) - s[0]
        }
        (head - silent).coerceAtLeast(0).toULong()
    }

    override fun setRate(rate: Float): Boolean = synchronized(lock) {
        this.rate = rate
        applyRateLocked()
        true
    }

    override fun closeOutput() = synchronized(lock) { releaseLocked() }

    private fun applyRateLocked() {
        val t = track ?: return
        // 变速不变调
        runCatching { t.playbackParams = PlaybackParams().setSpeed(rate).setPitch(1f) }
            .onFailure { Log.w(TAG, "setPlaybackParams failed", it) }
    }

    private fun resetCountersLocked() {
        lastRawHead = track?.playbackHeadPosition?.toLong()?.and(0xFFFF_FFFFL) ?: 0L
        wraps = 0
        // flush 后 head 一般归零；若不归零，把当前值当作起点
        streamWritten = lastRawHead
        silences.clear()
        if (lastRawHead > 0) silences.add(longArrayOf(0, lastRawHead))
        lastHeadChange = SystemClock.elapsedRealtime()
    }

    private fun releaseLocked() {
        track?.let {
            runCatching { it.pause(); it.flush() }
            it.release()
        }
        track = null
        playing = false
    }

    companion object {
        private const val TAG = "AudioTrackSink"
    }
}
