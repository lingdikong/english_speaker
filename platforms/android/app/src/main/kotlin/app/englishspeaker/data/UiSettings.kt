package app.englishspeaker.data

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

enum class ThemeMode { System, Light, Dark }

/**
 * 客户端设置（UI_SPEC §4.7：主题与阅读设置保存在客户端）。
 * 播放页“阅读设置” Sheet 与设置 Tab 共用这一组值。
 */
class UiSettings(context: Context) {
    private val prefs = context.getSharedPreferences("ui", Context.MODE_PRIVATE)

    private fun <T> flow(initial: T) = MutableStateFlow(initial)

    private val _theme = flow(runCatching { ThemeMode.valueOf(prefs.getString("theme", null) ?: "System") }.getOrDefault(ThemeMode.System))
    val theme: StateFlow<ThemeMode> = _theme
    /** 当前句字号，标准 24，范围 20–30。 */
    private val _fontSize = flow(prefs.getInt("fontSize", 24))
    val fontSize: StateFlow<Int> = _fontSize
    /** 行距：舒适 1.6 / 紧凑 1.4。 */
    private val _lineHeight = flow(prefs.getFloat("lineHeight", 1.6f))
    val lineHeight: StateFlow<Float> = _lineHeight
    /** 非当前句亮度：0.5 / 0.6 / 0.7（只改 other 的 alpha）。 */
    private val _otherAlpha = flow(prefs.getFloat("otherAlpha", 0.6f))
    val otherAlpha: StateFlow<Float> = _otherAlpha
    private val _focusMode = flow(prefs.getBoolean("focusMode", true))
    val focusMode: StateFlow<Boolean> = _focusMode
    private val _defaultRate = flow(prefs.getFloat("defaultRate", 1.0f))
    val defaultRate: StateFlow<Float> = _defaultRate

    fun setTheme(v: ThemeMode) { _theme.value = v; prefs.edit().putString("theme", v.name).apply() }
    fun setFontSize(v: Int) { _fontSize.value = v.coerceIn(20, 30); prefs.edit().putInt("fontSize", _fontSize.value).apply() }
    fun setLineHeight(v: Float) { _lineHeight.value = v; prefs.edit().putFloat("lineHeight", v).apply() }
    fun setOtherAlpha(v: Float) { _otherAlpha.value = v; prefs.edit().putFloat("otherAlpha", v).apply() }
    fun setFocusMode(v: Boolean) { _focusMode.value = v; prefs.edit().putBoolean("focusMode", v).apply() }
    fun setDefaultRate(v: Float) { _defaultRate.value = v; prefs.edit().putFloat("defaultRate", v).apply() }
}
