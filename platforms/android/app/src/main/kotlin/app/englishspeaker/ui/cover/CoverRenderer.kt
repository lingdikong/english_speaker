package app.englishspeaker.ui.cover

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.RectF
import app.englishspeaker.ui.components.Wave
import java.io.ByteArrayOutputStream

/**
 * 媒体通知 / 锁屏用的封面位图：与界面里的生成封面同一算法（UI_SPEC §3.1，种子 = 标题），
 * card 底（夜间）+ 一排 accent 竖条声波。按 72 缩略图的比例放大。
 */
object CoverRenderer {
    private val cache = HashMap<String, ByteArray>()

    @Suppress("UNUSED_PARAMETER")
    fun png(context: Context, id: String, title: String, size: Int = 512): ByteArray = synchronized(cache) {
        cache.getOrPut("$id|$title") {
            val bmp = render(title, size)
            ByteArrayOutputStream().also { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
        }
    }

    fun render(title: String, size: Int): Bitmap {
        val k = size / 72f
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val cv = Canvas(bmp)
        cv.drawColor(0xFF1E2330.toInt())
        val min = Wave.jsRound(72 / 10.0)
        val max = 72 - Wave.jsRound(72 / 2.6)
        val pad = Wave.jsRound(72 / 7.0) * k
        val hs = Wave.bars(Wave.seedOf(title), 11, min, max)
        val gap = 2 * k
        val w = (size - pad * 2 - gap * (hs.size - 1)) / hs.size
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFF3CCFB9.toInt() }
        hs.forEachIndexed { i, h ->
            val hh = h * k
            val x = pad + i * (w + gap)
            val y = (size - hh) / 2
            cv.drawRoundRect(RectF(x, y, x + w, y + hh), 2 * k, 2 * k, paint)
        }
        return bmp
    }
}
